// Installs the exact package/version pairs a benchmark probe needs, in an
// isolated temporary project, and verifies what actually landed against what
// the pinned manifest expects.
//
// Two things matter more here than convenience:
//
// - Lifecycle scripts must never run. The benchmark installs arbitrary
//   real-world packages by the thousand; any one of them could carry a
//   postinstall script, and running it would execute untrusted code on this
//   machine. `--ignore-scripts` is therefore not optional and every call site
//   is expected to keep it.
// - The lockfile is the only thing that lets `readLockIntegrity` prove what
//   was actually fetched. Bun's global package cache is intentionally kept
//   warm between probes; the exact specs and lockfile still pin the artifact.

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";

import { packageIntegrity } from "../../lib/package-integrity.mjs";

export function buildInstallArguments({ specs }) {
  return ["install", "--ignore-scripts", "--no-progress", ...specs];
}

// A frozen install reads the dependency set from package.json and every
// resolution from bun.lock, so it consults no registry manifest; the specs
// are already in the cached package.json.
export function buildFrozenInstallArguments() {
  return ["install", "--ignore-scripts", "--no-progress", "--frozen-lockfile"];
}

function sortedOverrides(overrides = {}) {
  return Object.fromEntries(Object.keys(overrides).sort().map(name => [name, overrides[name]]));
}

/// Where a probe's resolved install is remembered. The key is the exact spec
/// set, so a manifest re-pin (a new version) is a different entry, never a
/// stale hit.
///
/// The key also covers the project's `overrides`, because they decide the
/// resolution as much as the specs do: an entry resolved without the
/// `@solidjs/signals` pin is not an answer for a project that carries it, and a
/// key over the specs alone would hand that entry back and drop the pin
/// silently. A project with no overrides keeps the spec-only key it always had,
/// so every entry written before overrides existed still answers exactly the
/// installs it answered.
export function installLockfileCacheEntry(cacheRoot, specs, overrides = {}) {
  const sortedSpecs = [...specs].sort();
  const pinned = sortedOverrides(overrides);
  const material = Object.keys(pinned).length === 0
    ? JSON.stringify(sortedSpecs)
    : JSON.stringify({ specs: sortedSpecs, overrides: pinned });
  const key = createHash("sha256").update(material).digest("hex");
  return join(cacheRoot, "v1", key.slice(0, 2), key);
}

/// Bun's lockfile is JSON with trailing commas. Commas are removed only
/// outside strings and only directly before a closing bracket, so package
/// metadata containing punctuation is untouched.
export function parseBunLock(source) {
  if (typeof source !== "string") return null;
  let normalized = "";
  let inString = false;
  let escaped = false;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    if (inString) {
      normalized += character;
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') inString = false;
      continue;
    }
    if (character === '"') {
      inString = true;
      normalized += character;
      continue;
    }
    if (character === ",") {
      let next = index + 1;
      while (next < source.length && /\s/.test(source[next])) next += 1;
      if (source[next] === "}" || source[next] === "]") continue;
    }
    normalized += character;
  }
  try {
    const parsed = JSON.parse(normalized);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

/// Every installed record of each named package in a parsed Bun lock, by the
/// identity Bun recorded (`name@version`), wherever in the tree it sits. A
/// nested copy (`solid-js/@solidjs/signals`) is a record of its own, which is
/// what lets a pin be held to "exactly one copy".
export function lockCopies(lock, names) {
  const copies = Object.fromEntries(names.map(name => [name, []]));
  for (const [locator, value] of Object.entries(lock?.packages ?? {})) {
    if (!Array.isArray(value) || typeof value[0] !== "string") continue;
    const at = value[0].lastIndexOf("@");
    if (at <= 0) continue;
    const name = value[0].slice(0, at);
    if (!(name in copies)) continue;
    copies[name].push({
      locator,
      version: value[0].slice(at + 1),
      integrity: typeof value[3] === "string" ? value[3] : null
    });
  }
  for (const list of Object.values(copies)) list.sort((a, b) => a.locator.localeCompare(b.locator));
  return copies;
}

/// Whether a lock resolved without `overrides` already resolves every
/// overridden package exactly at its override. Only then is adding the
/// overrides to it a restatement rather than a change of resolution.
export function lockAgreesWithOverrides(lock, overrides) {
  if (!lock || lock.overrides !== undefined) return false;
  const copies = lockCopies(lock, Object.keys(overrides));
  return Object.entries(overrides).every(([name, version]) =>
    copies[name].every(copy => copy.version === version)
  );
}

/// The same lock text with an `overrides` block, spelled the way Bun writes one
/// (between `workspaces` and `packages`), so a frozen install accepts it and a
/// later ordinary install rewrites it byte for byte. `null` when the text is
/// not in the shape this knows how to extend.
export function lockTextWithOverrides(lockText, overrides) {
  const anchor = '\n  "packages": {';
  const at = lockText.indexOf(anchor);
  if (at < 0 || lockText.indexOf(anchor, at + 1) >= 0) return null;
  const pinned = sortedOverrides(overrides);
  const body = Object.entries(pinned)
    .map(([name, version]) => `    ${JSON.stringify(name)}: ${JSON.stringify(version)},\n`)
    .join("");
  return `${lockText.slice(0, at)}\n  "overrides": {\n${body}  },${lockText.slice(at)}`;
}

async function storeInstallLockfile(entry, projectDir) {
  const staging = `${entry}.staging-${process.pid}-${Date.now()}`;
  try {
    await mkdir(staging, { recursive: true });
    await copyFile(join(projectDir, "package.json"), join(staging, "package.json"));
    await copyFile(join(projectDir, "bun.lock"), join(staging, "bun.lock"));
    await mkdir(dirname(entry), { recursive: true });
    // A refresh replaces a stale entry: rename cannot land on a non-empty
    // directory, so the previous entry goes first. A reader racing this sees
    // a miss, never a torn entry.
    await rm(entry, { recursive: true, force: true });
    await rename(staging, entry);
  } catch {
    // Best-effort: a cache that cannot be written only costs the next run a
    // registry round trip per install. A concurrent writer's identical entry
    // winning the rename is the common reason to land here.
    await rm(staging, { recursive: true, force: true });
  }
}

export async function createProject({ root, specs, overrides = {} }) {
  // The dependency set is irrelevant here -- `specs` are passed directly as
  // Bun's install targets -- but `private: true` keeps Bun from ever treating
  // this throwaway probe directory as something publishable, and `overrides`
  // is the one resolution input a spec cannot carry: it is how a transitive
  // package (`@solidjs/signals` under `solid-js`) is pinned to one release.
  const pkg = { name: "solid-checker-ecosystem-probe", version: "0.0.0", private: true };
  if (Object.keys(overrides).length) pkg.overrides = sortedOverrides(overrides);
  await writeFile(join(root, "package.json"), `${JSON.stringify(pkg, null, 2)}\n`, "utf8");
  return { root, specs, overrides };
}

function packageJsonPath(projectDir, name) {
  return join(projectDir, "node_modules", ...name.split("/"), "package.json");
}

export function readInstalledVersions(projectDir, names) {
  const result = {};
  for (const name of names) {
    try {
      const raw = readFileSync(packageJsonPath(projectDir, name), "utf8");
      const parsed = JSON.parse(raw);
      result[name] = typeof parsed.version === "string" ? parsed.version : null;
    } catch {
      result[name] = null;
    }
  }
  return result;
}

export function readLockIntegrity(projectDir, names) {
  return Object.fromEntries(names.map(name => [name, packageIntegrity(projectDir, name)]));
}

/// What `from` resolves `name` to, by Node's lookup: the nearest
/// `node_modules/<name>` walking up from `from`'s real installed directory,
/// never above the project. `hoistedPath` is the project's own top-level copy,
/// so "resolves the pinned copy" is `path === hoistedPath`.
export function resolveInstalledDependency(projectDir, from, name) {
  const fromRoot = join(projectDir, "node_modules", ...from.split("/"));
  if (!existsSync(join(fromRoot, "package.json"))) return { from, fromInstalled: false };
  const top = realpathSync(projectDir);
  const hoisted = join(projectDir, "node_modules", ...name.split("/"));
  const hoistedPath = existsSync(join(hoisted, "package.json")) ? realpathSync(hoisted) : null;
  let directory = realpathSync(fromRoot);
  while (directory.startsWith(top)) {
    if (basename(directory) !== "node_modules") {
      const candidate = join(directory, "node_modules", ...name.split("/"));
      if (existsSync(join(candidate, "package.json"))) {
        let version = null;
        try {
          version = JSON.parse(readFileSync(join(candidate, "package.json"), "utf8")).version ?? null;
        } catch {
          version = null;
        }
        return { from, fromInstalled: true, path: realpathSync(candidate), version, hoistedPath };
      }
    }
    if (directory === top) break;
    directory = dirname(directory);
  }
  return { from, fromInstalled: true, path: null, version: null, hoistedPath };
}

/// The facts `verifyInstall` holds a probe's pins to, read from the installed
/// project: every lock record of every pinned package, and, for a pin that
/// names the package it must be resolved from, what that package resolves.
export function readPinnedInstallation(projectDir, pins = {}) {
  const names = Object.keys(pins);
  if (names.length === 0) return { lockRead: true, copies: {}, resolutions: {} };
  let lock = null;
  try {
    lock = parseBunLock(readFileSync(join(projectDir, "bun.lock"), "utf8"));
  } catch {
    lock = null;
  }
  const resolutions = {};
  for (const [name, pin] of Object.entries(pins)) {
    if (pin?.resolvedFrom) resolutions[name] = resolveInstalledDependency(projectDir, pin.resolvedFrom, name);
  }
  return { lockRead: lock !== null, copies: lockCopies(lock, names), resolutions };
}

/// A probe's pins against what landed. `pins` is
/// `{ [name]: { version, integrity, required, resolvedFrom? } }`:
///
/// - every installed copy of a pinned package must be the pinned version with
///   the pinned integrity, and there must be at most one (exactly one when the
///   pin is `required`) -- two copies mean some importer is not reading the
///   bytes the pin names;
/// - a pin with `resolvedFrom` must be the copy that package resolves, when
///   that package is installed at all.
///
/// Facts that were never gathered fail closed: a pinned probe whose install
/// did not report them is unverified, not verified.
export function verifyPins(pins = {}, pinned = null) {
  const problems = [];
  const names = Object.keys(pins).sort();
  if (names.length === 0) return problems;
  if (!pinned || pinned.lockRead !== true) {
    return names.map(name => ({ kind: "pin-unverified", package: name, expectedVersion: pins[name].version }));
  }
  for (const name of names) {
    const pin = pins[name];
    const copies = pinned.copies?.[name] ?? [];
    if (copies.length === 0) {
      if (pin.required) problems.push({ kind: "missing", package: name, expectedVersion: pin.version });
      continue;
    }
    if (copies.length > 1) {
      problems.push({
        kind: "duplicate-copies",
        package: name,
        expectedVersion: pin.version,
        locators: copies.map(copy => `${copy.locator}=${copy.version}`)
      });
    }
    for (const copy of copies) {
      if (copy.version !== pin.version) {
        problems.push({
          kind: "version-mismatch",
          package: name,
          expectedVersion: pin.version,
          actualVersion: copy.version,
          locator: copy.locator
        });
      } else if (pin.integrity && copy.integrity !== pin.integrity) {
        problems.push({
          kind: "integrity-mismatch",
          package: name,
          expectedIntegrity: pin.integrity,
          actualIntegrity: copy.integrity,
          locator: copy.locator
        });
      }
    }
    if (pin.resolvedFrom) {
      const resolution = pinned.resolutions?.[name];
      if (!resolution) {
        problems.push({ kind: "pin-unverified", package: name, expectedVersion: pin.version, from: pin.resolvedFrom });
      } else if (
        resolution.fromInstalled &&
        (resolution.path === null ||
          resolution.path !== resolution.hoistedPath ||
          resolution.version !== pin.version)
      ) {
        problems.push({
          kind: "unresolved-pin",
          package: name,
          from: pin.resolvedFrom,
          expectedVersion: pin.version,
          actualVersion: resolution.version ?? null
        });
      }
    }
  }
  return problems;
}

// `expected` is `{ [name]: { version, integrity } }` — the values pinned in
// the manifest. An integrity mismatch is reported exactly like a version
// mismatch: it is never treated as a softer or ignorable condition, because a
// changed tarball for the same version string is the exact tamper/republish
// case integrity pinning exists to catch.
export function verifyInstall({ expected, versions, integrity, pins = {}, pinned = null }) {
  const problems = [];
  for (const [name, want] of Object.entries(expected)) {
    const actualVersion = versions[name] ?? null;
    if (actualVersion === null) {
      problems.push({ kind: "missing", package: name, expectedVersion: want.version ?? null });
      continue;
    }
    if (want.version && actualVersion !== want.version) {
      problems.push({
        kind: "version-mismatch",
        package: name,
        expectedVersion: want.version,
        actualVersion
      });
    }
    const actualIntegrity = integrity[name] ?? null;
    if (want.integrity && actualIntegrity !== want.integrity) {
      problems.push({
        kind: "integrity-mismatch",
        package: name,
        expectedIntegrity: want.integrity,
        actualIntegrity
      });
    }
  }
  // A package both pinned and expected (a row that names `@solidjs/signals`
  // itself) can report the same mismatch twice; the report needs it once.
  const seen = new Set(problems.map(problem => JSON.stringify(problem)));
  for (const problem of verifyPins(pins, pinned)) {
    const key = JSON.stringify(problem);
    if (!seen.has(key)) {
      seen.add(key);
      problems.push(problem);
    }
  }
  problems.sort((a, b) => (a.package === b.package ? a.kind.localeCompare(b.kind) : a.package.localeCompare(b.package)));
  return { ok: problems.length === 0, problems };
}

// Real Bun invocation, used only when the caller does not inject `spawnImpl`.
// Kept isolated behind the injection point in `installPackages` so tests can
// exercise every install-result path (success, failure, timeout) without
// ever spawning a real Bun process or touching the network.
function defaultSpawn({ cwd, args, timeoutMs }) {
  return new Promise(resolve => {
    const child = spawn("bun", args, { cwd, env: process.env, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let spawnError = "";
    // A ChildProcess that emits `error` with no listener throws an uncaught
    // exception, which would take the whole benchmark down when Bun is simply
    // absent from PATH. `close` still fires after a failed spawn, so the
    // listener only has to record why, and the probe becomes one
    // install-failure result instead of a harness crash.
    child.on("error", error => {
      spawnError = `${error.code ?? "spawn error"}: ${error.message}`;
    });
    const timer = timeoutMs
      ? setTimeout(() => {
          timedOut = true;
          child.kill("SIGKILL");
        }, timeoutMs)
      : null;
    child.stdout.on("data", chunk => {
      stdout += chunk;
    });
    child.stderr.on("data", chunk => {
      stderr += chunk;
    });
    child.on("close", status => {
      if (timer) clearTimeout(timer);
      resolve({
        status,
        stdout,
        stderr: spawnError ? `${stderr}${stderr ? "\n" : ""}${spawnError}` : stderr,
        timedOut
      });
    });
  });
}

/// The spec-only entry a pinned install can inherit from, staged into the
/// project with the overrides added, or `null` when it cannot be inherited.
///
/// Every entry written before overrides existed is keyed on its specs alone,
/// and so is a miss for the same install once it carries a pin. A miss
/// re-resolves the whole tree against today's registry, which would move every
/// transitive package that has shipped since, not only the pinned one. When the
/// spec-only lock already resolves every overridden package exactly at its
/// override, adding the overrides restates that lock rather than changing it,
/// so it is staged instead -- and still only installed frozen, so Bun itself
/// confirms the lock satisfies the pinned package.json before anything runs.
async function stageInheritedLockfile(legacy, projectDir, overrides) {
  if (!existsSync(join(legacy, "bun.lock")) || !existsSync(join(legacy, "package.json"))) return null;
  let lockText;
  let pkg;
  try {
    lockText = await readFile(join(legacy, "bun.lock"), "utf8");
    pkg = JSON.parse(await readFile(join(legacy, "package.json"), "utf8"));
  } catch {
    return null;
  }
  if (!pkg || typeof pkg !== "object" || pkg.overrides !== undefined) return null;
  if (!lockAgreesWithOverrides(parseBunLock(lockText), overrides)) return null;
  const pinnedLock = lockTextWithOverrides(lockText, overrides);
  if (pinnedLock === null) return null;
  await writeFile(
    join(projectDir, "package.json"),
    `${JSON.stringify({ ...pkg, overrides: sortedOverrides(overrides) }, null, 2)}\n`,
    "utf8"
  );
  await writeFile(join(projectDir, "bun.lock"), pinnedLock, "utf8");
  return true;
}

/// Installs `specs` into `projectDir`. With `lockfileCache`, a previous run's
/// resolved package.json and bun.lock for the same exact spec set and
/// overrides are placed in the project first and Bun installs frozen: the
/// transitive resolution is the one recorded, no registry manifest is
/// consulted, and the tarballs come from Bun's own cache. What the harness
/// verifies afterwards -- the installed version and lock integrity of every
/// expected package against the manifest, and every pin -- is unchanged, and a
/// frozen install that fails for any reason falls back to the ordinary install,
/// whose result refreshes the entry. A pinned install with no entry of its own
/// inherits the spec-only entry when that entry already agrees with the pins
/// (`lockfileReuse: "inherited"`), and stores the result under its own key.
/// Without the cache (or on a miss) the install resolves against the registry
/// exactly as before, and a successful resolution is stored.
export async function installPackages({
  projectDir,
  specs,
  overrides = {},
  spawnImpl,
  timeoutMs,
  lockfileCache = null
}) {
  const run = spawnImpl ?? defaultSpawn;
  const pinned = Object.keys(overrides).length > 0;
  const entry = lockfileCache ? installLockfileCacheEntry(lockfileCache, specs, overrides) : null;
  const legacy = lockfileCache && pinned ? installLockfileCacheEntry(lockfileCache, specs) : null;
  const restart = async () => {
    await rm(join(projectDir, "bun.lock"), { force: true });
    await createProject({ root: projectDir, specs, overrides });
  };
  if (entry && existsSync(join(entry, "bun.lock")) && existsSync(join(entry, "package.json"))) {
    try {
      await copyFile(join(entry, "package.json"), join(projectDir, "package.json"));
      await copyFile(join(entry, "bun.lock"), join(projectDir, "bun.lock"));
      const frozen = await run({ cwd: projectDir, args: buildFrozenInstallArguments(), timeoutMs });
      if (frozen.status === 0 || frozen.timedOut) return { ...frozen, lockfileReuse: "hit" };
    } catch {
      // fall through to the ordinary install
    }
    await restart();
  } else if (legacy) {
    try {
      if (await stageInheritedLockfile(legacy, projectDir, overrides)) {
        const frozen = await run({ cwd: projectDir, args: buildFrozenInstallArguments(), timeoutMs });
        if (frozen.timedOut) return { ...frozen, lockfileReuse: "inherited" };
        if (frozen.status === 0) {
          await storeInstallLockfile(entry, projectDir);
          return { ...frozen, lockfileReuse: "inherited" };
        }
        await restart();
      }
    } catch {
      await restart();
    }
  }
  const result = await run({ cwd: projectDir, args: buildInstallArguments({ specs }), timeoutMs });
  if (entry && result.status === 0 && existsSync(join(projectDir, "bun.lock"))) {
    await storeInstallLockfile(entry, projectDir);
  }
  return { ...result, lockfileReuse: entry ? "miss" : null };
}

export async function withTemporaryProject(fn) {
  const dir = await mkdtemp(join(tmpdir(), "solid-checker-ecosystem-"));
  try {
    return await fn(dir);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}
