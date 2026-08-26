import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const VERSION = "1.9.14";
const CACHE_ROOT =
  process.env.SOLID_CHECKER_CONTRACT_RUNTIME_CACHE ??
  join(ROOT, "rust/target/contract-test-runtime", `solid-js-${VERSION}`);
const LOCK_ROOT = `${CACHE_ROOT}.lock`;
const READY_FILE = join(CACHE_ROOT, ".ready.json");
const FAILED_FILE = join(CACHE_ROOT, ".failed.json");
const FAILURE_TTL_MS = 5 * 60 * 1000;
const LOCK_TTL_MS = 10 * 60 * 1000;

function pause(milliseconds) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function packageVersion() {
  try {
    return JSON.parse(readFileSync(join(CACHE_ROOT, "node_modules/solid-js/package.json"), "utf8")).version;
  } catch {
    return null;
  }
}

function cacheReady() {
  try {
    const ready = JSON.parse(readFileSync(READY_FILE, "utf8"));
    return ready.version === VERSION && packageVersion() === VERSION;
  } catch {
    return false;
  }
}

function cachedFailure() {
  try {
    const failure = JSON.parse(readFileSync(FAILED_FILE, "utf8"));
    return failure.version === VERSION && Date.now() - failure.createdAt < FAILURE_TTL_MS
      ? failure.message
      : null;
  } catch {
    return null;
  }
}

function acquireLock() {
  mkdirSync(dirname(CACHE_ROOT), { recursive: true });
  while (true) {
    try {
      mkdirSync(LOCK_ROOT);
      writeFileSync(join(LOCK_ROOT, "owner"), `${process.pid}\n`);
      return;
    } catch (error) {
      if (error?.code !== "EEXIST") throw error;
      try {
        if (Date.now() - statSync(LOCK_ROOT).mtimeMs > LOCK_TTL_MS) {
          rmSync(LOCK_ROOT, { recursive: true, force: true });
          continue;
        }
      } catch {
        // The owner can disappear between mkdir and stat; retry normally.
      }
      pause(50);
    }
  }
}

function ensureCache() {
  if (cacheReady()) return { ok: true };
  const previousFailure = cachedFailure();
  if (previousFailure) return { ok: false, message: previousFailure };
  acquireLock();
  try {
    if (cacheReady()) return { ok: true };
    const lockedFailure = cachedFailure();
    if (lockedFailure) return { ok: false, message: lockedFailure };
    mkdirSync(CACHE_ROOT, { recursive: true });
    writeFileSync(
      join(CACHE_ROOT, "package.json"),
      JSON.stringify({ name: "solid-checker-contract-runtime", private: true }) + "\n"
    );
    const install = spawnSync(
      "bun",
      [
        "install",
        "--ignore-scripts",
        "--no-progress",
        "--no-save",
        `solid-js@${VERSION}`
      ],
      { cwd: CACHE_ROOT, encoding: "utf8", timeout: 300_000 }
    );
    if (install.status !== 0 || packageVersion() !== VERSION) {
      const message =
        (install.stderr ?? install.error?.message ?? `bun exited with ${install.status}`).trim();
      writeFileSync(
        FAILED_FILE,
        `${JSON.stringify({ version: VERSION, createdAt: Date.now(), message })}\n`
      );
      return {
        ok: false,
        message
      };
    }
    rmSync(FAILED_FILE, { force: true });
    writeFileSync(READY_FILE, `${JSON.stringify({ version: VERSION })}\n`);
    return { ok: true };
  } finally {
    rmSync(LOCK_ROOT, { recursive: true, force: true });
  }
}

/**
 * Makes the exact audited Solid release available to an isolated test
 * project. The package is installed once per checkout/cache and copied into
 * each temporary project, so parallel contract tests never race Bun or share
 * mutable package files.
 */
export function installAuditedSolid(directory) {
  const result = ensureCache();
  if (!result.ok) return result;
  const target = join(directory, "node_modules/solid-js");
  mkdirSync(dirname(target), { recursive: true });
  cpSync(join(CACHE_ROOT, "node_modules/solid-js"), target, { recursive: true, force: true });
  return { ok: existsSync(join(target, "package.json")) };
}
