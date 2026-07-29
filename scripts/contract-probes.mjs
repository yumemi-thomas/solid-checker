#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import * as solid from "solid-js";
import * as web from "@solidjs/web";

const request = JSON.parse(process.argv[2]);

function entrypointLeaves(target, conditions = []) {
  if (typeof target === "string") {
    return /\.m?js$/.test(target) ? [{ target, conditions }] : [];
  }
  if (!target || typeof target !== "object") return [];
  return Object.entries(target).flatMap(([condition, value]) =>
    entrypointLeaves(value, condition === "default" ? conditions : [...conditions, condition]),
  );
}

const packages = {};
for (const item of request.packages) {
  const manifest = JSON.parse(readFileSync(join(item.directory, "package.json"), "utf8"));
  const entrypoints = {};
  for (const [entrypoint, target] of Object.entries(manifest.exports ?? {})) {
    if (entrypoint.includes("*")) continue;
    const kinds = {};
    const conditions = new Set();
    for (const leaf of entrypointLeaves(target)) {
      const path = join(item.directory, leaf.target);
      if (!existsSync(path)) continue;
      leaf.conditions.forEach(condition => conditions.add(condition));
      const module = await import(pathToFileURL(path));
      for (const [name, value] of Object.entries(module)) {
        if (name === "default") continue;
        const kind = typeof value === "function" ? "function" : "value";
        if (kinds[name] && kinds[name] !== kind) {
          throw new Error(
            `${item.name}${entrypoint} exports ${name} with inconsistent runtime kinds`,
          );
        }
        kinds[name] = kind;
      }
    }
    if (Object.keys(kinds).length > 0) {
      entrypoints[entrypoint] = {
        exports: kinds,
        conditions: [...conditions].sort(),
      };
    }
  }
  packages[item.name] = { version: manifest.version, entrypoints };
}

const probes = [];
function probe(pkg, entrypoint, name, claim, body) {
  let ok = false;
  let error;
  try {
    solid.createRoot(dispose => {
      ok = Boolean(body());
      dispose();
    });
    solid.flush();
  } catch (caught) {
    error = String(caught);
  }
  probes.push({ pkg, entrypoint, name, claim, ok, ...(error ? { error } : {}) });
}

probe("solid-js", ".", "createMemo", "returns=accessor", () => {
  const memo = solid.createMemo(() => 1);
  return typeof memo === "function" && memo() === 1;
});

probe("solid-js", ".", "createMemo", "callbacks[0]=tracked", () => {
  const [source, setSource] = solid.createSignal(0);
  let runs = 0;
  const memo = solid.createMemo(() => {
    runs++;
    return source();
  });
  memo();
  const before = runs;
  setSource(1);
  solid.flush();
  memo();
  return runs > before;
});

function probeSplitEffect(pkg, name, create) {
  probe(pkg, ".", name, "callbacks[0]=tracked", () => {
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
    setSource(1);
    solid.flush();
    return runs > before;
  });
  probe(pkg, ".", name, "callbacks[1]=deferred", () => {
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
    const before = applyRuns;
    setOther(1);
    solid.flush();
    return applyRuns === before;
  });
}

probeSplitEffect("solid-js", "createEffect", solid.createEffect);
probeSplitEffect("solid-js", "createRenderEffect", solid.createRenderEffect);
probeSplitEffect("@solidjs/web", "effect", web.effect);

probe("solid-js", ".", "createTrackedEffect", "callbacks[0]=tracked", () => {
  const [source, setSource] = solid.createSignal(0);
  let runs = 0;
  solid.createTrackedEffect(() => {
    runs++;
    source();
  });
  solid.flush();
  const before = runs;
  setSource(1);
  solid.flush();
  return runs > before;
});

probe("solid-js", ".", "children", "returns=accessor", () => {
  return typeof solid.children(() => "child") === "function";
});

probe("solid-js", ".", "mapArray", "returns=accessor", () => {
  const [list] = solid.createSignal([1, 2]);
  const mapped = solid.mapArray(list, value => value * 2);
  return typeof mapped === "function" && JSON.stringify(mapped()) === "[2,4]";
});

probe("solid-js", ".", "createProjection", "returns=store-path", () => {
  const projection = solid.createProjection(draft => {
    draft.value = 1;
  }, { value: 0 });
  return projection?.value === 1;
});

probe("solid-js", ".", "createProjection", "callbacks[0]=tracked", () => {
  const [source, setSource] = solid.createSignal(1);
  let runs = 0;
  const projection = solid.createProjection(draft => {
    runs++;
    draft.value = source();
  }, { value: 0 });
  const before = runs;
  setSource(2);
  solid.flush();
  return projection.value === 2 && runs > before;
});

probe("solid-js", ".", "onSettled", "callbacks[0]=deferred", () => {
  const [source, setSource] = solid.createSignal(0);
  let runs = 0;
  solid.onSettled(() => {
    runs++;
    source();
  });
  solid.flush();
  if (runs !== 1) return false;
  setSource(1);
  solid.flush();
  return runs === 1;
});

probe("solid-js", ".", "createRoot", "callbacks[0]=inline", () => {
  let ran = false;
  solid.createRoot(() => {
    ran = true;
  });
  return ran;
});

probe("solid-js", ".", "runWithOwner", "callbacks[1]=inline", () => {
  let ran = false;
  solid.runWithOwner(solid.getOwner(), () => {
    ran = true;
  });
  return ran;
});

probe("@solidjs/web", ".", "memo", "returns=accessor", () => {
  const memo = web.memo(() => 1);
  return typeof memo === "function" && memo() === 1;
});

probe("@solidjs/web", ".", "memo", "callbacks[0]=tracked", () => {
  const [source, setSource] = solid.createSignal(0);
  let runs = 0;
  const memo = web.memo(() => {
    runs++;
    return source();
  });
  memo();
  const before = runs;
  setSource(1);
  solid.flush();
  memo();
  return runs > before;
});

process.stdout.write(JSON.stringify({ packages, probes }));
