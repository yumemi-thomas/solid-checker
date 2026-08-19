#!/usr/bin/env node
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

async function probeSplitEffect(pkg, name, create) {
  await probe(pkg, ".", name, "callbacks[0]=tracked", () => {
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
  await probe(pkg, ".", name, "callbacks[1]=deferred", () => {
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

await probeSplitEffect("solid-js", "createEffect", solid.createEffect);
await probeSplitEffect("solid-js", "createRenderEffect", solid.createRenderEffect);
await probeSplitEffect("@solidjs/web", "effect", web.effect);

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

emit(packages, probes);
