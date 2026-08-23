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
//   was actually fetched. `--no-package-lock` would remove that evidence, so
//   it must never be added even though some npm invocations use it to speed
//   up throwaway installs.

import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

export function buildInstallArguments({ specs }) {
  return ["install", "--ignore-scripts", "--no-audit", "--no-fund", "--loglevel=error", ...specs];
}

export async function createProject({ root, specs }) {
  // The package.json content itself is irrelevant to what gets installed —
  // `specs` are passed directly as npm's install targets — but `private:
  // true` keeps npm from ever treating this throwaway probe directory as
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
  const result = {};
  let lock = null;
  try {
    lock = JSON.parse(readFileSync(join(projectDir, "package-lock.json"), "utf8"));
  } catch {
    lock = null;
  }
  for (const name of names) {
    const entry = lock?.packages?.[`node_modules/${name}`];
    result[name] = typeof entry?.integrity === "string" ? entry.integrity : null;
  }
  return result;
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

// Real npm invocation, used only when the caller does not inject `spawnImpl`.
// Kept isolated behind the injection point in `installPackages` so tests can
// exercise every install-result path (success, failure, timeout) without
// ever spawning a real npm process or touching the network.
function defaultSpawn({ cwd, args, timeoutMs }) {
  return new Promise(resolve => {
    const child = spawn("npm", args, { cwd, env: process.env, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let spawnError = "";
    // A ChildProcess that emits `error` with no listener throws an uncaught
    // exception, which would take the whole benchmark down when npm is simply
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

export async function installPackages({ projectDir, specs, spawnImpl, timeoutMs }) {
  const run = spawnImpl ?? defaultSpawn;
  const args = buildInstallArguments({ specs });
  return run({ cwd: projectDir, args, timeoutMs });
}

export async function withTemporaryProject(fn) {
  const dir = await mkdtemp(join(tmpdir(), "solid-checker-ecosystem-"));
  try {
    return await fn(dir);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}
