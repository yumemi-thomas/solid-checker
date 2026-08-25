#!/usr/bin/env node
// Exact @solid-primitives/debounce@1.3.0 callback probes. Both exports create
// the same timer-backed wrapper; creation requires a Solid owner for cleanup,
// while the user callback runs later from the platform timer without one.
import { createRoot } from "solid-js";
import debounce, { createDebounce } from "@solid-primitives/debounce";

import { createRecorder, describePackages, emit } from "./lib/contract-probe-harness.mjs";

const request = JSON.parse(process.argv[2]);
const mode = request.mode ?? "unspecified";
const packages = await describePackages(request);
const DEBOUNCE = "@solid-primitives/debounce";
const settle = () => new Promise(resolve => setTimeout(resolve, 25));

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
  ["createDebounce", createDebounce],
  ["default", debounce],
]) {
  await probe(DEBOUNCE, ".", name, "callbacks[0]=deferred", async () => {
    let inside = true;
    let runs = 0;
    const trigger = create(() => {
      if (!inside) runs++;
    }, 0);
    inside = false;
    if (runs !== 0) return false;
    trigger();
    if (runs !== 0) return false;
    await settle();
    return runs === 1;
  }, 2);
}

emit(packages, probes);
