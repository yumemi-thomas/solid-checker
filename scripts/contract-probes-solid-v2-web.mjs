#!/usr/bin/env bun
// Isolated @solidjs/web subpath probes. Importing the JSX and frames runtimes
// installs renderer-wide trace hooks, so these must not share the core Solid
// worker whose claims require the root server renderer's untouched state.
import * as solid from "solid-js";
import * as webJsx from "@solidjs/web/jsx-runtime";
import * as webFramesServer from "@solidjs/web/frames/server";
import { provideRequestEvent } from "@solidjs/web/storage";

import { createRecorder, describePackages, emit } from "./lib/contract-probe-harness.mjs";

const request = JSON.parse(process.argv[2]);
const mode = request.mode ?? "unspecified";
const isServer = mode === "server";
const packages = await describePackages(request);

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

function writeOutsideOwner(setter, value) {
  return solid.runWithOwner(null, () => setter(value));
}

for (const entrypoint of ["./jsx-runtime", "./jsx-dev-runtime"]) {
  await probe("@solidjs/web", entrypoint, "applyRef", "callbacks[0]=inline", () => {
    let ran = false;
    webJsx.applyRef(() => {
      ran = true;
    }, {});
    return ran;
  });
  await probe("@solidjs/web", entrypoint, "createComponent", "callbacks[0]=inline", () => {
    let ran = false;
    webJsx.createComponent(() => {
      ran = true;
      return undefined;
    }, {});
    return ran;
  });
  await probe("@solidjs/web", entrypoint, "getNextElement", "callbacks[0]=inline", () => {
    let ran = false;
    webJsx.getNextElement(() => {
      ran = true;
      return {};
    });
    return ran;
  });
  await probe("@solidjs/web", entrypoint, "untrack", "callbacks[0]=inline", () => {
    let ran = false;
    webJsx.untrack(() => {
      ran = true;
    });
    return ran;
  });

  const dynamicExecution = isServer ? "deferred" : "tracked";
  await probe(
    "@solidjs/web",
    entrypoint,
    "dynamic",
    `callbacks[0]=${dynamicExecution}`,
    () => {
      let runs = 0;
      const Component = webJsx.dynamic(() => {
        runs++;
        return () => undefined;
      });
      if (runs !== 0) return false;
      const rendered = Component({});
      if (typeof rendered === "function") rendered();
      return runs === 1;
    },
  );

  await probe("@solidjs/web", entrypoint, "memo", "returns=accessor", () => {
    const value = webJsx.memo(() => 1);
    return typeof value === "function" && value() === 1;
  });
  await probe(
    "@solidjs/web",
    entrypoint,
    "memo",
    `callbacks[0]=${isServer ? "inline" : "tracked"}`,
    () => {
      const [source, setSource] = solid.createSignal(0);
      let runs = 0;
      const value = webJsx.memo(() => {
        runs++;
        return source();
      });
      value();
      if (runs !== 1) return false;
      const before = runs;
      writeOutsideOwner(setSource, 1);
      solid.flush();
      value();
      return isServer ? runs === before : runs > before;
    },
    isServer ? 1 : 2,
  );

  if (isServer) {
    for (const parameter of [0, 1]) {
      await probe("@solidjs/web", entrypoint, "effect", `callbacks[${parameter}]=inline`, () => {
        let computeRuns = 0;
        let applyRuns = 0;
        webJsx.effect(
          () => {
            computeRuns++;
            return 1;
          },
          () => {
            applyRuns++;
          },
        );
        return parameter === 0 ? computeRuns === 1 : applyRuns === 1;
      });
    }
  } else {
    await probe("@solidjs/web", entrypoint, "effect", "callbacks[0]=tracked", () => {
      const [source, setSource] = solid.createSignal(0);
      let runs = 0;
      webJsx.effect(
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
    await probe("@solidjs/web", entrypoint, "effect", "callbacks[1]=deferred", () => {
      const [source] = solid.createSignal(0);
      const [other, setOther] = solid.createSignal(0);
      let applyRuns = 0;
      webJsx.effect(
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
}

if (isServer) {
  await probe("@solidjs/web", "./storage", "provideRequestEvent", "callbacks[1]=inline", () => {
    let ran = false;
    provideRequestEvent({}, () => {
      ran = true;
    });
    return ran;
  });
}

for (const entrypoint of ["./frames", "./frames/server"]) {
  if (entrypoint === "./frames" && !isServer) continue;
  await probe("@solidjs/web", entrypoint, "serverComponentResponse", "callbacks[0]=inline", async () => {
    let ran = false;
    const response = webFramesServer.serverComponentResponse(() => {
      ran = true;
      return "component";
    });
    await response.body?.cancel();
    return ran;
  });
  await probe("@solidjs/web", entrypoint, "frameTransformResult", "callbacks[1]=inline", async () => {
    let ran = false;
    const response = webFramesServer.frameTransformResult({}, () => {
      ran = true;
      return "component";
    });
    await response.body?.cancel();
    return ran;
  });
}

emit(packages, probes);
