#!/usr/bin/env bun
// Exact @solid-primitives/rootless@1.5.4 callback timing probes. The package's
// root wrappers either invoke their callback during the exported call, or
// return a function whose later invocation creates/enters the root.
import { createRoot } from "solid-js";
import {
  createBranch,
  createCallback,
  createDisposable,
  createHydratableSingletonRoot,
  createRootPool,
  createSharedRoot,
  createSingletonRoot,
  createSubRoot,
} from "@solid-primitives/rootless";

import { createRecorder, describePackages, emit } from "./lib/contract-probe-harness.mjs";

const request = JSON.parse(process.argv[2]);
const mode = request.mode ?? "unspecified";
const packages = await describePackages(request);
const ROOTLESS = "@solid-primitives/rootless";

const { probes, probe } = createRecorder({
  mode,
  runInRoot: async body => {
    let dispose = () => {};
    try {
      return await createRoot(async disposer => {
        dispose = disposer;
        return await body();
      });
    } finally {
      dispose();
    }
  },
});

for (const [name, create] of [
  ["createBranch", createBranch],
  ["createDisposable", createDisposable],
  ["createSubRoot", createSubRoot],
]) {
  await probe(ROOTLESS, ".", name, "callbacks[0]=inline", () => {
    let inside = true;
    let runs = 0;
    create(() => {
      if (inside) runs++;
    });
    inside = false;
    return runs === 1;
  });
}

await probe(ROOTLESS, ".", "createCallback", "callbacks[0]=deferred", () => {
  let runs = 0;
  const callback = createCallback(() => runs++);
  if (runs !== 0) return false;
  callback();
  return runs === 1;
});

for (const [name, create] of [
  ["createHydratableSingletonRoot", createHydratableSingletonRoot],
  ["createSharedRoot", createSharedRoot],
  ["createSingletonRoot", createSingletonRoot],
]) {
  await probe(ROOTLESS, ".", name, "callbacks[0]=deferred", () => {
    let runs = 0;
    const use = create(() => ++runs);
    if (runs !== 0) return false;
    use();
    return runs === 1;
  });
}

await probe(ROOTLESS, ".", "createRootPool", "callbacks[0]=deferred", () => {
  let runs = 0;
  const use = createRootPool(() => ++runs);
  if (runs !== 0) return false;
  use(1);
  return runs === 1;
});

emit(packages, probes);
