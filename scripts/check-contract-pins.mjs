#!/usr/bin/env bun
// Verifies that every bundled contract names a package release that exists in
// the registry, with the exact tarball the contract was audited against.
//
// `scripts/check-bundled-contracts.mjs` already proves this for contracts it
// probes: it installs them and reads Bun's lockfile. That leaves the
// contracts it does not probe -- a hand-authored overlay, or a dialect whose
// runtime is not probed at all -- pinned by a version string nothing checks.
// A version string alone is not a pin: republished or mutated contents keep
// the same version, and the contract would still claim to describe them.
//
// So an absent integrity is a failure here, not a skip. A pin that cannot be
// falsified is not a pin.
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { loadDialectManifests, root } from "./dialect-manifests.mjs";
import {
  cacheEnabled,
  hashFile,
  runtimeIdentity,
  scriptClosure,
  writeJsonAtomic,
} from "./lib/gate-cache.mjs";

let failures = 0;
const fail = message => {
  failures++;
  console.error(`FAIL ${message}`);
};

/** The registry's integrity for one exact release, or an explained failure. */
function registryIntegrity(name, version) {
  const result = spawnSync("bun", ["info", `${name}@${version}`, "dist.integrity", "--json"], {
    cwd: join(root, "packages/cli"),
    encoding: "utf8",
  });
  if (result.error) {
    return { error: `cannot run bun info: ${result.error.message}` };
  }
  if (result.status !== 0) {
    const detail = result.stderr.trim().split("\n").at(-1) ?? `exit ${result.status}`;
    return { error: `registry lookup failed: ${detail}` };
  }
  const output = result.stdout.trim();
  if (output === "" || output === "undefined") {
    return { error: "the registry reports no release at that exact version" };
  }
  let parsed;
  try {
    parsed = JSON.parse(output);
  } catch (error) {
    return { error: `unreadable bun info output: ${error.message}` };
  }
  // A range or tag would yield an array; an exact version must not.
  if (typeof parsed !== "string") {
    return { error: `expected one integrity, got ${JSON.stringify(parsed)}` };
  }
  return { integrity: parsed };
}

// A local memo of registry answers, keyed by the exact `name@version` asked
// about, so a repeated local run does not repeat the network round trip.
//
// This memo stores the *falsifier itself* -- the ground truth `verifyPin`
// compares a bundled pin against -- so it gets the same treatment the gate
// result caches get, and for a sharper reason: a result cache that replays a
// stale value produces a wrong verdict, and this one would produce a wrong
// verdict about whether a pin can still be falsified at all. Hence:
//
//   * **A format version and an input digest.** An entry is readable only by a
//     reader whose inputs produced it: the memo schema, this script's own
//     closure (so changing `registryIntegrity` -- a different Bun field, a
//     different invocation, a bug fix -- misses instead of replaying answers
//     the new logic never asked for), the Bun runtime version, and the *effective
//     registry*, resolved the way npm resolves it for this invocation. Warm the
//     memo against a mirror, switch back to npmjs, and every entry misses.
//   * **Strict shape validation on read.** Anything that is not exactly the
//     expected envelope -- a wrong format, a foreign digest, a non-string
//     entry, an unrecognized top-level field, a malformed `name@version` key --
//     discards the whole file. A file that cannot be understood completely is
//     not partially trusted.
//   * **A memoized answer is candidate data, not the verdict.** `lookup` is
//     given the pin it is about to be compared against; an entry that
//     *disagrees* triggers a live lookup before the gate is allowed to fail.
//     So a stale or hand-edited entry cannot invent a MISMATCH, and cannot make
//     one out of a republish that happened while the memo was warm.
//
// What this still cannot do, stated plainly: a hand-edited entry that agrees
// with the bundled pin is indistinguishable from a live answer that agrees, so
// a memo in a user-writable build root is not tamper-proof and is not claimed
// to be. What keeps that bounded is where it lives and does not live:
//
//   * `rust/target/` -- a build root, wiped by `make clean`, and never present
//     in CI's `contracts` job (it builds no Rust), so every push and pull
//     request still performs the live registry lookup for every pin;
//   * `SOLID_CHECKER_GATE_CACHE=0` disables it, the same switch the gate result
//     caches use;
//   * only a successful lookup is memoized -- an error is never stored, so a
//     transient network failure cannot become a permanent pass.
//
// The comparison against the checked-in expectation runs every time either way;
// this only skips re-asking the registry the same immutable question.
const MEMO = join(root, "rust/target/registry-integrity.json");

/** Bumped when a stored entry's *meaning* changes; older entries stop being readable. */
export const MEMO_FORMAT_VERSION = 1;

const MEMO_FIELDS = ["formatVersion", "inputDigest", "entries"];
// `name@version`, scoped names included. A key this does not match is a key
// nothing in this file could have written.
const MEMO_KEY = /^(?:@[^@/\s]+\/)?[^@/\s]+@[^@\s]+$/;
const MEMO_INTEGRITY = /^sha\d{3}-[A-Za-z0-9+/]+={0,2}$/;

/**
 * The registry Bun would actually talk to for this invocation.
 *
 * Bun accepts the npm-compatible registry environment variables. Its CLI has
 * no config-get equivalent, so an explicitly configured registry is honored;
 * otherwise use Bun's documented default. Keeping this value in the memo key
 * prevents a mirror's answer from serving a later npmjs.org run.
 */
export function effectiveRegistry(env = process.env) {
  return (
    env.BUN_CONFIG_REGISTRY ??
    env.NPM_CONFIG_REGISTRY ??
    env.npm_config_registry ??
    "https://registry.npmjs.org/"
  );
}

/** The digest binding a stored entry to the inputs that could have produced it. */
export function memoInputDigest({ registry, scriptPath = fileURLToPath(import.meta.url) } = {}) {
  const closure = scriptClosure(scriptPath, { root });
  const hash = createHash("sha256");
  for (const part of [
    `memo-format:${MEMO_FORMAT_VERSION}`,
    `memo-fields:${MEMO_FIELDS.join(",")}`,
    runtimeIdentity,
    `registry:${registry}`,
    ...closure.files.map((path) => `script:${relative(root, path)}:${hashFile(path)}`),
    `script-closure-uncertain:${closure.uncertain}`,
  ]) {
    hash.update(part);
    hash.update("\0");
  }
  return hash.digest("hex");
}

/**
 * Every entry in a stored memo, or `{}` when the file is anything other than
 * exactly what this version writes against these inputs.
 */
export function readMemo(file, inputDigest) {
  if (!existsSync(file)) return {};
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(file, "utf8"));
  } catch {
    return {};
  }
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) return {};
  if (Object.keys(parsed).some((field) => !MEMO_FIELDS.includes(field))) return {};
  if (parsed.formatVersion !== MEMO_FORMAT_VERSION) return {};
  if (typeof parsed.inputDigest !== "string" || parsed.inputDigest !== inputDigest) return {};
  const entries = parsed.entries;
  if (entries === null || typeof entries !== "object" || Array.isArray(entries)) return {};
  for (const [id, integrity] of Object.entries(entries)) {
    if (!MEMO_KEY.test(id)) return {};
    if (typeof integrity !== "string" || !MEMO_INTEGRITY.test(integrity)) return {};
  }
  return { ...entries };
}

export function memoizedIntegrity(
  lookup = registryIntegrity,
  { file = MEMO, enabled = cacheEnabled(), registry = undefined } = {},
) {
  // `registry: null` states "unresolvable" explicitly; `undefined` means ask.
  const resolvedRegistry = enabled ? (registry === undefined ? effectiveRegistry() : registry) : null;
  // An unresolvable registry is not a reason to trust the file anyway.
  const active = enabled && resolvedRegistry !== null;
  const inputDigest = active ? memoInputDigest({ registry: resolvedRegistry }) : null;
  const store = active ? readMemo(file, inputDigest) : {};
  let hits = 0;
  let misses = 0;
  let refuted = 0;
  let dirty = false;
  return {
    /**
     * The registry's integrity for one release.
     *
     * `expected` is the pin this answer is about to be compared against. A
     * memoized answer that matches it is served; one that does not is *not*
     * served, because the gate would then fail on the memo's word rather than
     * the registry's. Miss, live lookup, then verdict.
     */
    lookup(name, version, expected) {
      const id = `${name}@${version}`;
      const memoized = store[id];
      if (active && typeof memoized === "string") {
        if (expected === undefined || memoized === expected) {
          hits += 1;
          return { integrity: memoized };
        }
        // The memo would fail this pin. Only the registry gets to do that.
        refuted += 1;
      }
      const observed = lookup(name, version);
      misses += 1;
      if (active && typeof observed.integrity === "string" && MEMO_INTEGRITY.test(observed.integrity)) {
        store[id] = observed.integrity;
        dirty = true;
      }
      return observed;
    },
    /**
     * Merge-on-save, not last-writer-wins: the file is re-read and merged so a
     * concurrent run's entries survive. Losing one would only cost a future
     * live lookup, but the merge is two lines and removes the question.
     */
    flush() {
      if (!active || !dirty) return;
      writeJsonAtomic(file, {
        formatVersion: MEMO_FORMAT_VERSION,
        inputDigest,
        entries: { ...readMemo(file, inputDigest), ...store },
      });
    },
    summary() {
      if (!enabled) return "registry memo: disabled (SOLID_CHECKER_GATE_CACHE)";
      if (!active) return "registry memo: disabled (the effective npm registry could not be resolved)";
      const refutedNote = refuted > 0 ? `, ${refuted} memoized answer(s) re-checked live` : "";
      return `registry memo: ${hits} hit(s), ${misses} live lookup(s)${refutedNote}`;
    },
  };
}

/**
 * Checks one contract's pin, returning `undefined` when it holds and the
 * failure sentence when it does not. `lookup` is the registry query, injected
 * so the rules can be tested without a network.
 *
 * The pin's own integrity is handed to `lookup` as well. A live query ignores
 * it; the memo uses it to tell "this cached answer agrees, serve it" from "this
 * cached answer would fail the gate, ask the registry instead". The verdict
 * below is drawn from whatever `lookup` returns either way.
 */
export function verifyPin({ label, file, expectedName, document }, lookup = registryIntegrity) {
  const pin = document?.package ?? {};
  if (pin.name !== expectedName) {
    return `${label}: ${file} describes ${JSON.stringify(pin.name)}, not ${expectedName}`;
  }
  if (typeof pin.version !== "string" || pin.version === "") {
    return `${label}: ${file} records no package version`;
  }
  if (typeof pin.integrity !== "string" || pin.integrity === "") {
    return (
      `${label}: ${file} pins ${pin.name}@${pin.version} by version alone. Record the release's ` +
      `integrity (bun info ${pin.name}@${pin.version} dist.integrity) so the pin can be falsified.`
    );
  }
  const observed = lookup(pin.name, pin.version, pin.integrity);
  if (observed.error) return `${label}: ${pin.name}@${pin.version} ${observed.error}`;
  if (observed.integrity !== pin.integrity) {
    return (
      `${label}: ${pin.name}@${pin.version} is ${observed.integrity} in the registry, but the ` +
      `contract was audited against ${pin.integrity}. The artifact this contract describes is not ` +
      `the one the registry now serves.`
    );
  }
  return undefined;
}

function main() {
  const contracts = loadDialectManifests({ requireArtifacts: true }).flatMap(manifest =>
    manifest.contracts.map(contract => ({ dialect: manifest.id, ...contract })),
  );
  const memo = memoizedIntegrity();
  for (const contract of contracts) {
    const label = `${contract.dialect}/${contract.package}`;
    const failure = verifyPin(
      {
        label,
        file: contract.bundledContract,
        expectedName: contract.package,
        document: JSON.parse(readFileSync(join(root, contract.bundledContract), "utf8")),
      },
      (name, version, expected) => memo.lookup(name, version, expected),
    );
    if (failure) {
      fail(failure);
      continue;
    }
    console.log(`ok   ${label}: matches its audited tarball`);
  }
  memo.flush();
  console.log(memo.summary());
  if (failures > 0) {
    console.error(`${failures} bundled contract pin(s) could not be verified`);
    process.exit(1);
  }
  console.log(`verified ${contracts.length} bundled contract pins against the registry`);
}

if (fileURLToPath(import.meta.url) === resolve(process.argv[1] ?? "")) main();
