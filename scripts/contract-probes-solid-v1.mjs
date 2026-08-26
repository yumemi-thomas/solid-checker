#!/usr/bin/env bun
// Solid 1.x contract probe worker. Runs inside the solid-v1 install directory,
// so `solid-js` resolves to the audited 1.9.14 release rather than the 2.0
// prerelease the other dialect pins. Shared machinery lives in
// scripts/lib/contract-probe-harness.mjs.
//
// 1.x has no `flush()`. Its effects run on a microtask queue, so a probe body
// settles by awaiting a macrotask instead — which is also what the scheduled
// primitives need, since their whole subject is timer-backed deferral.
import { createRoot } from "solid-js";
import { isServer } from "solid-js/web";
import * as scheduled from "@solid-primitives/scheduled";

import { createRecorder, describePackages, emit } from "./lib/contract-probe-harness.mjs";

const request = JSON.parse(process.argv[2]);
const mode = request.mode ?? "unspecified";

const packages = await describePackages(request);

const settle = (ms = 25) => new Promise(resolve => setTimeout(resolve, ms));

const { probes, probe } = createRecorder({
  mode,
  runInRoot: async body => {
    let result;
    let dispose = () => {};
    try {
      result = await createRoot(async disposer => {
        dispose = disposer;
        return await body();
      });
    } finally {
      dispose();
    }
    return result;
  },
});

const SCHEDULED = "@solid-primitives/scheduled";

/**
 * `deferred` on a scheduler factory means the callback is not invoked while the
 * factory call runs — so a reactive read inside it is never tracked by the
 * caller. That is the claim, and it holds in every condition.
 *
 * The stronger evidence, that the callback does run later, is only available
 * off the server: `isServer` selects a branch that returns a no-op and never
 * calls the callback at all. Asserting "runs later" unconditionally would fail
 * in the server mode against a package that is behaving exactly as documented,
 * so the strong half is asserted where the runtime actually schedules.
 */
function probeDeferredScheduler(name, create) {
  return probe(
    SCHEDULED,
    ".",
    name,
    "callbacks[0]=deferred",
    async () => {
      let runs = 0;
      const trigger = create(() => {
        runs++;
      });
      if (runs !== 0) return false;
      trigger();
      if (runs !== 0) return false;
      await settle();
      return isServer ? runs === 0 : runs === 1;
    },
    2,
  );
}

await probeDeferredScheduler("debounce", callback => scheduled.debounce(callback, 0));
await probeDeferredScheduler("throttle", callback => scheduled.throttle(callback, 0));
await probeDeferredScheduler("scheduleIdle", callback => scheduled.scheduleIdle(callback, 0));

/**
 * `leading` and `leadingAndTrailing` take the scheduler factory first and the
 * user callback second. The factory is invoked while the export call runs
 * (inline); the callback is not (deferred) — it runs on the returned trigger.
 *
 * On the server neither holds the same way: that branch never calls the factory
 * at all, which is why those exports carry per-condition variants and this
 * worker only probes the inline claim where the contract states it.
 */
function probeLeadingEdge(name, create) {
  const inlineClaim = probe(
    SCHEDULED,
    ".",
    name,
    "callbacks[0]=inline",
    () => {
      let factoryCalls = 0;
      create(
        (callback, wait) => {
          factoryCalls++;
          return scheduled.debounce(callback, wait);
        },
        () => {},
      );
      return factoryCalls === 1;
    },
    1,
  );
  const deferredClaim = probe(
    SCHEDULED,
    ".",
    name,
    "callbacks[1]=deferred",
    async () => {
      let runs = 0;
      const trigger = create(
        (callback, wait) => scheduled.debounce(callback, wait),
        () => {
          runs++;
        },
      );
      // Not invoked while the export call runs. The leading edge fires on the
      // trigger, which is a later call and not what this claim describes.
      if (runs !== 0) return false;
      trigger();
      await settle();
      return runs >= 1;
    },
    2,
  );
  return Promise.all([inlineClaim, deferredClaim]);
}

if (!isServer) {
  await probeLeadingEdge("leading", (schedule, callback) =>
    scheduled.leading(schedule, callback, 0),
  );
  await probeLeadingEdge("leadingAndTrailing", (schedule, callback) =>
    scheduled.leadingAndTrailing(schedule, callback, 0),
  );
} else {
  // The server variants state only the deferred claim, so only it is probed.
  for (const [name, create] of [
    ["leading", scheduled.leading],
    ["leadingAndTrailing", scheduled.leadingAndTrailing],
  ]) {
    await probe(
      SCHEDULED,
      ".",
      name,
      "callbacks[1]=deferred",
      async () => {
        let runs = 0;
        const trigger = create(
          (callback, wait) => scheduled.debounce(callback, wait),
          () => {
            runs++;
          },
          0,
        );
        if (runs !== 0) return false;
        trigger();
        await settle();
        return runs >= 1;
      },
      2,
    );
  }
}

await probe(
  SCHEDULED,
  ".",
  "createScheduled",
  "callbacks[0]=inline",
  () => {
    let factoryCalls = 0;
    scheduled.createScheduled(callback => {
      factoryCalls++;
      return scheduled.debounce(callback, 0);
    });
    return factoryCalls === 1;
  },
  1,
);

await probe(SCHEDULED, ".", "createScheduled", "returns=accessor", () => {
  const track = scheduled.createScheduled(callback => scheduled.debounce(callback, 0));
  return typeof track === "function" && typeof track() === "boolean";
});

emit(packages, probes);
