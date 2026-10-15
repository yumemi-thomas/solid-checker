// The owned check (P5 S2): a project checked from its files alone, with the
// canonical Type Facts producer and the checker both running as WebAssembly
// in one worker thread over one validated in-memory copy of the project.
// Node and Bun only. See README.md, "Owned check".
import { Worker } from "node:worker_threads";
import { loadArtifacts } from "./owned-artifacts.mjs";
import { attach } from "./owned-client.mjs";
import { mountBuffers, validateMount } from "./owned-mount.mjs";

export { OwnedCheckerError } from "./owned-client.mjs";
export { OwnedMountError, mountDirectory } from "./owned-mount.mjs";

/** Starts one owned checker over `mount`; resolves once both reactors are ready. */
export async function createOwnedChecker(options) {
  if (options === null || typeof options !== "object") {
    throw new TypeError("createOwnedChecker takes an options object");
  }
  for (const key of Object.keys(options)) {
    if (key !== "mount") throw new TypeError(`createOwnedChecker has no option ${JSON.stringify(key)}`);
  }
  const mount = validateMount(options.mount);
  const { checker, producer } = await loadArtifacts();
  const worker = new Worker(new URL("./owned-worker.mjs", import.meta.url));
  return attach(worker, { checker, producer, mount, transfer: [checker, producer, ...mountBuffers(mount)] });
}
