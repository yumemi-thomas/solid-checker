// The owned check (P5 S2, S3b): a project checked from its files alone, with
// the canonical Type Facts producer and the checker both running as
// WebAssembly in one worker thread over one filesystem -- a validated
// in-memory copy of the project (`mount`) or a host directory read on
// demand (`directory`). Node and Bun only. See README.md, "Owned check".
import { lstatSync, realpathSync } from "node:fs";
import { sep } from "node:path";
import { Worker } from "node:worker_threads";
import { loadArtifacts } from "./owned-artifacts.mjs";
import { attach } from "./owned-client.mjs";
import { mountBuffers, OwnedMountError, validateMount } from "./owned-mount.mjs";
import { identityOf } from "./owned-snapshot.mjs";

export { OwnedCheckerError } from "./owned-client.mjs";
export { OwnedMountError, mountDirectory } from "./owned-mount.mjs";

/**
 * Reads the options' one source key exactly once: the key list once, then
 * the present key's value once, into a value this function owns.
 */
function captureOptions(options) {
  if (options === null || typeof options !== "object") {
    throw new TypeError("createOwnedChecker takes an options object");
  }
  const keys = Reflect.ownKeys(options);
  for (const key of keys) {
    if (key !== "mount" && key !== "directory") {
      throw new TypeError(`createOwnedChecker has no option ${typeof key === "symbol" ? key.toString() : JSON.stringify(key)}`);
    }
  }
  if (keys.length > 1) throw new TypeError("createOwnedChecker takes exactly one of mount or directory");
  // No key at all is an absent mount, refused by its validation as before.
  if (keys.length === 0) return { key: "mount", value: undefined };
  return { key: keys[0], value: options[keys[0]] };
}

/** The canonical directory and its identity, captured on this thread. */
function captureDirectory(directory) {
  if (typeof directory !== "string" || directory === "" || directory.includes("\0")) {
    throw new TypeError("directory must be a non-empty path string");
  }
  if (sep !== "/") throw new OwnedMountError("directory supports POSIX hosts only");
  let root;
  let stats;
  try {
    root = realpathSync(directory);
    stats = lstatSync(root, { bigint: true });
  } catch (error) {
    throw new OwnedMountError(`directory ${directory} cannot be read: ${error.message}`);
  }
  if (!stats.isDirectory()) throw new OwnedMountError(`directory ${root} is not a directory`);
  if (root === "/") throw new OwnedMountError("directory must not be the filesystem root");
  return { input: directory, root, identity: identityOf(stats) };
}

/**
 * Starts one owned checker over `mount` or `directory`; resolves once both
 * reactors are ready.
 */
export async function createOwnedChecker(options) {
  const { key, value } = captureOptions(options);
  if (key === "directory") {
    const directory = captureDirectory(value);
    const { checker, producer } = await loadArtifacts();
    const worker = new Worker(new URL("./owned-worker.mjs", import.meta.url));
    return attach(worker, { checker, producer, directory, transfer: [checker, producer] });
  }
  const mount = validateMount(value);
  const { checker, producer } = await loadArtifacts();
  const worker = new Worker(new URL("./owned-worker.mjs", import.meta.url));
  return attach(worker, { checker, producer, mount, transfer: [checker, producer, ...mountBuffers(mount)] });
}
