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
import { existsSync, readFileSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

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

/// Where a probe's resolved install is remembered. The key is the exact spec
/// set, so a manifest re-pin (a new version) is a different entry, never a
/// stale hit.
export function installLockfileCacheEntry(cacheRoot, specs) {
  const key = createHash("sha256").update(JSON.stringify([...specs].sort())).digest("hex");
  return join(cacheRoot, "v1", key.slice(0, 2), key);
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

export async function createProject({ root, specs }) {
  // The package.json content itself is irrelevant to what gets installed —
  // `specs` are passed directly as Bun's install targets — but `private:
  // true` keeps Bun from ever treating this throwaway probe directory as
  // something publishable.
  const pkg = { name: "solid-checker-ecosystem-probe", version: "0.0.0", private: true };
  await writeFile(join(root, "package.json"), `${JSON.stringify(pkg, null, 2)}\n`, "utf8");
  return { root, specs };
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

// `expected` is `{ [name]: { version, integrity } }` — the values pinned in
// the manifest. An integrity mismatch is reported exactly like a version
// mismatch: it is never treated as a softer or ignorable condition, because a
// changed tarball for the same version string is the exact tamper/republish
// case integrity pinning exists to catch.
export function verifyInstall({ expected, versions, integrity }) {
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

/// Installs `specs` into `projectDir`. With `lockfileCache`, a previous run's
/// resolved package.json and bun.lock for the same exact spec set are placed in
/// the project first and Bun installs frozen: the transitive resolution is the
/// one recorded, no registry manifest is consulted, and the tarballs come from
/// Bun's own cache. What the harness verifies afterwards — the installed
/// version and lock integrity of every expected package against the manifest —
/// is unchanged, and a frozen install that fails for any reason falls back to
/// the ordinary install, whose result refreshes the entry. Without the cache
/// (or on a miss) the install resolves against the registry exactly as before,
/// and a successful resolution is stored.
export async function installPackages({ projectDir, specs, spawnImpl, timeoutMs, lockfileCache = null }) {
  const run = spawnImpl ?? defaultSpawn;
  const entry = lockfileCache ? installLockfileCacheEntry(lockfileCache, specs) : null;
  if (entry && existsSync(join(entry, "bun.lock")) && existsSync(join(entry, "package.json"))) {
    try {
      await copyFile(join(entry, "package.json"), join(projectDir, "package.json"));
      await copyFile(join(entry, "bun.lock"), join(projectDir, "bun.lock"));
      const frozen = await run({ cwd: projectDir, args: buildFrozenInstallArguments(), timeoutMs });
      if (frozen.status === 0 || frozen.timedOut) return { ...frozen, lockfileReuse: "hit" };
    } catch {
      // fall through to the ordinary install
    }
    await rm(join(projectDir, "bun.lock"), { force: true });
    await createProject({ root: projectDir, specs });
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
