#!/usr/bin/env bun
// Solid 2.0 contract probe worker. Runs inside the solid-v2 install directory,
// so `solid-js` and `@solidjs/web` resolve to that dialect's audited releases.
// Shared machinery lives in scripts/lib/contract-probe-harness.mjs.
import * as solid from "solid-js";
import * as web from "@solidjs/web";

import { createRecorder, describePackages, emit } from "./lib/contract-probe-harness.mjs";

const request = JSON.parse(process.argv[2]);
const mode = request.mode ?? "unspecified";

const packages = await describePackages(request);

// A 2.0 probe body runs under a disposable root and is settled with flush().
const { probes, probe } = createRecorder({
  mode,
  runInRoot: async body => {
    let result;
    await solid.createRoot(async dispose => {
      result = await body();
      dispose();
    });
    solid.flush();
    return result;
  },
});

// Development builds reject writes made from a parent-owned test root unless
// the source explicitly opts into ownedWrite. A probe represents an external
// event/update, so detach the write exactly as the runtime's public
// runWithOwner(null, ...) escape hatch does instead of changing the package's
// source-creation options just for the harness.
function writeOutsideOwner(setter, value) {
  return solid.runWithOwner(null, () => setter(value));
}

await probe("solid-js", ".", "createMemo", "returns=accessor", () => {
  const memo = solid.createMemo(() => 1);
  return typeof memo === "function" && memo() === 1;
});

await probe("solid-js", ".", "createMemo", "callbacks[0]=tracked", () => {
  const [source, setSource] = solid.createSignal(0);
  let runs = 0;
  const memo = solid.createMemo(() => {
    runs++;
    return source();
  });
  memo();
  const before = runs;
  writeOutsideOwner(setSource, 1);
  solid.flush();
  memo();
  return runs > before;
}, 2);

// The browser build's split-effect rows. The server build states different
// executions for the same two slots -- it runs both synchronously inside the
// call -- and is probed separately below; registering the browser bodies there
// would record a failing result for a claim no variant states in that mode.
async function probeSplitEffect(pkg, entrypoint, name, create) {
  if (mode === "server") return;
  await probe(pkg, entrypoint, name, "callbacks[0]=tracked", () => {
    const [source, setSource] = solid.createSignal(0);
    let runs = 0;
    create(
      () => {
        runs++;
        return source();
      },
      () => {},
    );
    solid.flush();
    const before = runs;
    writeOutsideOwner(setSource, 1);
    solid.flush();
    return runs > before;
  }, 2);
  await probe(pkg, entrypoint, name, "callbacks[1]=deferred", () => {
    const [source] = solid.createSignal(0);
    const [other, setOther] = solid.createSignal(0);
    let applyRuns = 0;
    create(
      () => source(),
      () => {
        applyRuns++;
        other();
      },
    );
    solid.flush();
    if (applyRuns !== 1) return false;
    const before = applyRuns;
    writeOutsideOwner(setOther, 1);
    solid.flush();
    return applyRuns === before;
  }, 2);
}

await probeSplitEffect("solid-js", ".", "createEffect", solid.createEffect);
await probeSplitEffect("solid-js", ".", "createRenderEffect", solid.createRenderEffect);
await probeSplitEffect("@solidjs/web", ".", "effect", web.effect);

await probe("solid-js", ".", "createTrackedEffect", "callbacks[0]=tracked", () => {
  const [source, setSource] = solid.createSignal(0);
  let runs = 0;
  solid.createTrackedEffect(() => {
    runs++;
    source();
  });
  solid.flush();
  const before = runs;
  writeOutsideOwner(setSource, 1);
  solid.flush();
  return runs > before;
}, 2);

await probe("solid-js", ".", "children", "returns=accessor", () => {
  return typeof solid.children(() => "child") === "function";
});

await probe("solid-js", ".", "mapArray", "returns=accessor", () => {
  const [list] = solid.createSignal([1, 2]);
  const mapped = solid.mapArray(list, value => value * 2);
  return typeof mapped === "function" && JSON.stringify(mapped()) === "[2,4]";
});

await probe("solid-js", ".", "createProjection", "returns=store-path", () => {
  const projection = solid.createProjection(draft => {
    draft.value = 1;
  }, { value: 0 });
  return projection?.value === 1;
});

await probe("solid-js", ".", "createProjection", "callbacks[0]=tracked", () => {
  const [source, setSource] = solid.createSignal(1);
  let runs = 0;
  const projection = solid.createProjection(draft => {
    runs++;
    draft.value = source();
  }, { value: 0 });
  const before = runs;
  writeOutsideOwner(setSource, 2);
  solid.flush();
  return projection.value === 2 && runs > before;
}, 2);

await probe("solid-js", ".", "onSettled", "callbacks[0]=deferred", () => {
  const [source, setSource] = solid.createSignal(0);
  let runs = 0;
  solid.onSettled(() => {
    runs++;
    source();
  });
  solid.flush();
  if (runs !== 1) return false;
  writeOutsideOwner(setSource, 1);
  solid.flush();
  return runs === 1;
}, 2);

await probe("solid-js", ".", "createRoot", "callbacks[0]=inline", () => {
  let ran = false;
  solid.createRoot(() => {
    ran = true;
  });
  return ran;
});

await probe("solid-js", ".", "runWithOwner", "callbacks[1]=inline", () => {
  let ran = false;
  solid.runWithOwner(solid.getOwner(), () => {
    ran = true;
  });
  return ran;
});

await probe("@solidjs/web", ".", "memo", "returns=accessor", () => {
  const memo = web.memo(() => 1);
  return typeof memo === "function" && memo() === 1;
});

await probe("@solidjs/web", ".", "memo", "callbacks[0]=tracked", () => {
  const [source, setSource] = solid.createSignal(0);
  let runs = 0;
  const memo = web.memo(() => {
    runs++;
    return source();
  });
  memo();
  const before = runs;
  writeOutsideOwner(setSource, 1);
  solid.flush();
  memo();
  return runs > before;
}, 2);

// ---------------------------------------------------------------------------
// The callback rows the 2026-08-23 discovery audit added.
//
// `inline`, `deferred` and `tracked` classify **attribution**, so each body
// below asks who owns the reads the callback performs rather than when it ran:
//
//   inline    the callback ran inside the export call, and either the call
//             site re-runs on a write (the wrapper preserves tracking) or the
//             wrapper explicitly clears the listener, in which case nothing
//             re-runs and the synchronous run is the whole observation.
//   tracked   the callback re-runs alone: the export gave it a subscription of
//             its own.
//   deferred  the callback did not run inside the export call, runs later, and
//             does not re-run.
//
// Solid 2.0's server build re-runs nothing at all, so a claim stated only for
// the browser variant is registered only in the client modes and vice versa.
// Registering it everywhere would record a failing result for a claim the
// contract never states in that mode.
// ---------------------------------------------------------------------------

const isServer = mode === "server";
const clientModes = !isServer;

/**
 * An export that runs its thunk inside the call while **clearing** the
 * listener: nothing subscribes and nothing re-runs, so the synchronous run is
 * the observation. Same shape as the existing `createRoot` probe.
 */
async function probeClearingInline(pkg, entrypoint, name, invoke) {
  await probe(pkg, entrypoint, name, "callbacks[0]=inline", () => {
    let ran = false;
    invoke(() => {
      ran = true;
      return undefined;
    });
    return ran;
  });
}

/**
 * An export that runs its thunk inside the call and **preserves** tracking:
 * the reads land on the call site, so an enclosing memo re-runs on a write.
 * On the server nothing re-runs, and the synchronous run is all there is.
 */
async function probeTransparentInline(pkg, entrypoint, name, invoke) {
  await probe(
    pkg,
    entrypoint,
    name,
    "callbacks[0]=inline",
    () => {
      const [source, setSource] = solid.createSignal(0);
      let runs = 0;
      let siteRuns = 0;
      const site = solid.createMemo(() => {
        siteRuns++;
        return invoke(() => {
          runs++;
          return source();
        });
      });
      site();
      if (runs !== 1) return false;
      if (isServer) return true;
      const before = siteRuns;
      writeOutsideOwner(setSource, 1);
      solid.flush();
      site();
      return siteRuns > before;
    },
    isServer ? 1 : 2,
  );
}

/**
 * A factory whose thunk is lazily forced through the value it returned.
 *
 * `tracked` is the browser claim: forcing the result again after a write
 * re-runs the thunk without the call site re-running. `inline` is the server
 * claim: the thunk runs once when the result is forced and never again,
 * because the server build has no reactive graph to re-enter.
 */
async function probeLazyFactory(name, execution, create, force = value => value()) {
  await probe(
    "solid-js",
    ".",
    name,
    `callbacks[0]=${execution}`,
    () => {
      const [source, setSource] = solid.createSignal(0);
      let runs = 0;
      let siteRuns = 0;
      const site = solid.createMemo(() => {
        siteRuns++;
        return create(() => {
          runs++;
          return source();
        });
      });
      const value = site();
      force(value);
      if (runs !== 1) return false;
      const beforeSite = siteRuns;
      writeOutsideOwner(setSource, 1);
      solid.flush();
      site();
      force(value);
      if (siteRuns > beforeSite) return false;
      return execution === "tracked" ? runs > 1 : runs === 1;
    },
    execution === "tracked" ? 2 : 1,
  );
}

// --- wrappers that clear the listener -------------------------------------
await probeClearingInline("solid-js", ".", "untrack", thunk => solid.untrack(thunk));
await probeClearingInline("solid-js", ".", "createRevealOrder", thunk =>
  solid.createRevealOrder(thunk),
);
await probeClearingInline("solid-js", ".", "createComponent", thunk =>
  solid.createComponent(thunk, {}),
);
await probeClearingInline("solid-js", ".", "runInServerComponentScope", thunk =>
  solid.runInServerComponentScope(thunk),
);
// `flush(fn)` runs its thunk inside a synchronous drain. The server build is
// `function flush() {}` -- no declared parameter, empty body -- so the server
// variant states no callback row and no probe is registered for it.
if (clientModes) {
  await probeClearingInline("solid-js", ".", "flush", thunk => solid.flush(thunk));
}

// --- wrappers that preserve tracking --------------------------------------
await probeTransparentInline("solid-js", ".", "latest", thunk => solid.latest(thunk));
await probeTransparentInline("solid-js", ".", "isPending", thunk => solid.isPending(thunk));
await probeTransparentInline("solid-js", ".", "flatten", thunk => solid.flatten(thunk));

// --- lazily forced factories ----------------------------------------------
await probeLazyFactory("children", isServer ? "inline" : "tracked", thunk =>
  solid.children(thunk),
);
await probeLazyFactory(
  "createSignal",
  isServer ? "inline" : "tracked",
  thunk => solid.createSignal(thunk),
  ([read]) => read(),
);
await probeLazyFactory(
  "createOptimistic",
  isServer ? "inline" : "tracked",
  thunk => solid.createOptimistic(thunk),
  ([read]) => read(),
);

/**
 * One callback slot of an export whose result has to be forced before the
 * callback runs at all, asserted against the attribution the contract states.
 *
 * `create(callback)` builds the export call; `force(result)` reads whatever it
 * returned. The callback reads one signal, and the enclosing memo is the call
 * site whose re-runs separate `inline` from `tracked`.
 *
 * `deferred` deliberately does **not** assert that a second force leaves the
 * count alone: forcing an uncached thunk again re-invokes the callback, which
 * is the reader calling it, not a subscription the export handed out.
 */
async function probeForcedCallback(name, parameter, execution, create, force) {
  await probe(
    "solid-js",
    ".",
    name,
    `callbacks[${parameter}]=${execution}`,
    () => {
      const [source, setSource] = solid.createSignal(0);
      let runs = 0;
      let siteRuns = 0;
      const site = solid.createMemo(() => {
        siteRuns++;
        return create(() => {
          runs++;
          return source();
        });
      });
      const result = site();
      const ranInsideTheCall = runs > 0;
      if (ranInsideTheCall !== (execution !== "deferred")) return false;
      force(result);
      if (runs < 1) return false;
      const beforeSite = siteRuns;
      const beforeRuns = runs;
      writeOutsideOwner(setSource, 1);
      solid.flush();
      // The call site must not have re-run: that would make the reads the
      // caller's, which is what `inline` claims and these do not.
      if (siteRuns > beforeSite) return false;
      if (execution === "deferred") return true;
      force(site());
      return execution === "tracked" ? runs > beforeRuns : runs === beforeRuns;
    },
    execution === "tracked" ? 2 : 1,
  );
}

// --- repeat(count, map) ----------------------------------------------------
// `count` runs inside the mapping computation and holds its own subscription
// on the browser. `map` runs with the listener cleared -- a row is created once
// per index and a signal it read never re-runs it -- so it is inline in both
// builds, which is what the runtime does rather than what the signature
// suggests.
await probeForcedCallback(
  "repeat",
  0,
  isServer ? "inline" : "tracked",
  thunk => solid.repeat(thunk, index => index),
  rows => rows(),
);
await probeForcedCallback(
  "repeat",
  1,
  "inline",
  thunk => solid.repeat(() => 1, thunk),
  rows => rows(),
);

// --- boundaries ------------------------------------------------------------
// The browser build evaluates the boundary's own computation eagerly, so both
// the body and the fallback run inside the call and both re-run alone: their
// reads belong to the boundary, not to the call site. The server build of
// createLoadingBoundary calls the body inline and reaches the fallback only
// from the thunk it returned; the server build of createErrorBoundary
// references neither argument outside that thunk, so both are deferred.
const pendingForever = () => new solid.NotReadyError(new Promise(() => {}));

for (const [name, create] of [
  ["createLoadingBoundary", (body, fallback) => solid.createLoadingBoundary(body, fallback)],
  ["createErrorBoundary", (body, fallback) => solid.createErrorBoundary(body, fallback)],
]) {
  const trip = () => {
    throw name === "createLoadingBoundary" ? pendingForever() : new Error("probe");
  };
  await probeForcedCallback(
    name,
    0,
    isServer ? (name === "createErrorBoundary" ? "deferred" : "inline") : "tracked",
    thunk => create(thunk, () => "fallback"),
    boundary => boundary(),
  );
  await probeForcedCallback(
    name,
    1,
    isServer ? "deferred" : "tracked",
    thunk => create(trip, thunk),
    boundary => boundary(),
  );
}

// --- the server build's split-effect and memo rows --------------------------
// `serverEffect` runs `compute(undefined)` and then `effectFn?.(result)` inside
// the call, in the caller's scope; the server build has no queue to defer to
// and nothing ever re-runs. `createEffect` passes `undefined` for `effectFn`,
// so its parameter 1 is genuinely never invoked there and states no row.
if (isServer) {
  for (const [pkg, entrypoint, name, create] of [
    ["solid-js", ".", "createMemo", solid.createMemo],
    ["@solidjs/web", ".", "memo", web.memo],
  ]) {
    await probe(pkg, entrypoint, name, "callbacks[0]=inline", () => {
      let runs = 0;
      const memo = create(() => {
        runs++;
        return 1;
      });
      memo();
      if (runs !== 1) return false;
      solid.flush();
      memo();
      return runs === 1;
    });
  }

  for (const [pkg, entrypoint, name, create] of [
    ["solid-js", ".", "createEffect", (compute, apply) => solid.createEffect(compute, apply)],
    [
      "solid-js",
      ".",
      "createRenderEffect",
      (compute, apply) => solid.createRenderEffect(compute, apply),
    ],
    ["@solidjs/web", ".", "effect", (compute, apply) => web.effect(compute, apply)],
  ]) {
    await probe(pkg, entrypoint, name, "callbacks[0]=inline", () => {
      let runs = 0;
      create(
        () => {
          runs++;
          return 1;
        },
        () => {},
      );
      return runs === 1;
    });
    if (name === "createEffect") continue;
    await probe(pkg, entrypoint, name, "callbacks[1]=inline", () => {
      let applyRuns = 0;
      create(
        () => 1,
        () => {
          applyRuns++;
        },
      );
      return applyRuns === 1;
    });
  }
}

emit(packages, probes);
