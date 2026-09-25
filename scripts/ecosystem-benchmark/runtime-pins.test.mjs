// The `@solidjs/signals` pins are only worth having if they are the bytes the
// audits and the cache already name. These tests hold them to three sources:
// the audited archives (ADR 0007), the shipped tier's own environments, and --
// when it is on disk -- the install-lockfile cache every corpus run reuses,
// which is what makes pinning a no-op for the pinned report.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import { loadAuditedArchives } from "./lib/dialect-authority.mjs";
import { installLockfileCacheEntry, lockCopies, parseBunLock } from "./lib/install.mjs";
import {
  SIGNALS_FLOOR,
  SIGNALS_HEAD,
  SOLID_SIGNALS_RELEASES,
  corpusSignalsPin
} from "./lib/runtime-pins.mjs";
import { satisfies } from "./lib/semver.mjs";
import { probeInstallPlan } from "./run.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const manifest = JSON.parse(readFileSync(join(ROOT, "scripts/ecosystem-benchmark/manifest.json"), "utf8"));
const LOCK_CACHE = join(ROOT, "rust/target/install-locks");

const solid2Probes = rows =>
  rows
    .filter(row => row.solidTarget === "solid2")
    .flatMap(row => row.probes.map(probe => ({ row, probe })));

test("the audited signals pins are ADR 0007's archives, in the JSON mirror and in solid_2.rs", () => {
  const archives = loadAuditedArchives().dialects.find(dialect => dialect.id === "solid-v2").archives;
  const rust = readFileSync(join(ROOT, "rust/crates/solid-dialect/src/solid_2.rs"), "utf8");
  for (const version of ["2.0.0-rc.3", "2.0.0-rc.6"]) {
    const archive = archives.find(entry => entry.name === "@solidjs/signals" && entry.version === version);
    assert.ok(archive, `${version} is an audited archive`);
    assert.equal(SOLID_SIGNALS_RELEASES[version], archive.integrity);
    const block = new RegExp(
      `name: "@solidjs/signals",\\s*version: "${version.replace(/\./g, "\\.")}",\\s*integrity: "([^"]+)"`
    ).exec(rust);
    assert.ok(block, `solid_2.rs AUDITED_ARCHIVES lists @solidjs/signals@${version}`);
    assert.equal(SOLID_SIGNALS_RELEASES[version], block[1]);
  }
  // The checked-in rc.6 package.json is the archive the integrity names.
  const rc6 = readFileSync(join(ROOT, "benchmarks/package-contract-v2/phase0/rc6/solidjs-signals/package.json"));
  assert.equal(JSON.parse(rc6.toString("utf8")).version, SIGNALS_HEAD);
  const archive = archives.find(entry => entry.name === "@solidjs/signals" && entry.version === SIGNALS_HEAD);
  assert.equal(createHash("sha256").update(rc6).digest("hex"), archive.manifestSha256);
});

test("every signals release the shipped tier was proven with is the pinned one", () => {
  const index = JSON.parse(readFileSync(join(ROOT, "pkg/contracts/accepted/index.json"), "utf8"));
  const seen = new Set();
  for (const bundle of index.bundles) {
    for (const entry of bundle.dependencyEnvironment ?? []) {
      if (entry.name !== "@solidjs/signals") continue;
      seen.add(entry.version);
      assert.equal(entry.integrity, SOLID_SIGNALS_RELEASES[entry.version], `${bundle.packageName} ${entry.version}`);
    }
  }
  // rc.0 has no audited archive; the tier's floor environments are one of the
  // two sources that attest its integrity.
  assert.ok(seen.has(SIGNALS_FLOOR), "the tier carries floor environments");
});

test("every Solid 2 probe's pin sits inside the signals range its solid-js declares", () => {
  const peers = manifest.solidReleases["solid-js"].peers;
  const probes = solid2Probes([...manifest.rows, ...(manifest.supplemental ?? [])]);
  assert.ok(probes.length > 200);
  for (const { row, probe } of probes) {
    const { pins, overrides } = probeInstallPlan(row, probe, manifest.solidReleases);
    const pin = pins["@solidjs/signals"];
    assert.ok(pin, probe.id);
    assert.equal(pin.integrity, SOLID_SIGNALS_RELEASES[pin.version]);
    assert.deepEqual(overrides, { "@solidjs/signals": pin.version }, "a corpus probe overrides signals and nothing else");
    const explicit = probe.solid["@solidjs/signals"];
    if (explicit) assert.equal(pin.version, explicit, `${probe.id} names signals itself`);
    const solidJs = probe.solid["solid-js"];
    const range = solidJs ? peers[solidJs]?.["@solidjs/signals"] : null;
    if (range) assert.ok(satisfies(pin.version, range), `${probe.id}: ${pin.version} outside solid-js@${solidJs}'s ${range}`);
  }
  // The rule, stated on the two tuples the census measures.
  assert.equal(corpusSignalsPin({ solid: { "solid-js": "2.0.0-rc.0", "@solidjs/web": "2.0.0-rc.0" } }).version, SIGNALS_FLOOR);
  assert.equal(corpusSignalsPin({ solid: { "solid-js": "2.0.0-rc.3", "@solidjs/web": "2.0.0-rc.3" } }).version, SIGNALS_HEAD);
  assert.equal(corpusSignalsPin({ solid: { "@solidjs/signals": "2.0.0-rc.9" } }), null, "an unpinned release refuses");
});

test("a Solid 1 probe pins nothing", () => {
  const row = manifest.rows.find(candidate => candidate.solidTarget === "solid1");
  const { pins, overrides } = probeInstallPlan(row, row.probes[0], manifest.solidReleases);
  assert.deepEqual(pins, {});
  assert.deepEqual(overrides, {});
});

// The byte-identity claim. Every corpus probe installs frozen from this cache,
// so a pin that disagreed with what a cached lock resolves would either fail
// verification or, through the lockfile inheritance, never be inherited and
// re-resolve against today's registry. Skipped where there is no cache (CI).
test.skipIf(!existsSync(join(LOCK_CACHE, "v1")))(
  "the install-lockfile cache resolves exactly the pinned signals release for every cached probe",
  () => {
    const disagreements = [];
    let checked = 0;
    let uncached = 0;
    for (const { row, probe } of solid2Probes(manifest.rows)) {
      const { specs, pins, overrides } = probeInstallPlan(row, probe, manifest.solidReleases);
      const entry = [installLockfileCacheEntry(LOCK_CACHE, specs, overrides), installLockfileCacheEntry(LOCK_CACHE, specs)]
        .find(candidate => existsSync(join(candidate, "bun.lock")));
      if (!entry) {
        uncached += 1;
        continue;
      }
      checked += 1;
      const lock = parseBunLock(readFileSync(join(entry, "bun.lock"), "utf8"));
      const pin = pins["@solidjs/signals"];
      const copies = lockCopies(lock, ["@solidjs/signals"])["@solidjs/signals"];
      if (copies.length !== 1 || copies[0].version !== pin.version || copies[0].integrity !== pin.integrity) {
        disagreements.push(`${probe.id}: pinned ${pin.version}, cached ${copies.map(copy => `${copy.locator}=${copy.version}`).join(", ") || "none"}`);
      }
    }
    assert.deepEqual(disagreements, []);
    assert.ok(checked > 0, `the cache exists but answers none of the manifest's Solid 2 probes (${uncached} uncached)`);
  }
);
