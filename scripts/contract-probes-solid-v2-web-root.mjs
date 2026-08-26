#!/usr/bin/env bun
// Root @solidjs/web probes that mutate renderer-wide server context are kept
// out of the core Solid worker and the JSX/frames worker.
import * as solid from "solid-js";
import * as web from "@solidjs/web";

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

await probe("@solidjs/web", ".", "applyRef", "callbacks[0]=inline", () => {
  let ran = false;
  web.applyRef(() => {
    ran = true;
  }, {});
  return ran;
});
await probe("@solidjs/web", ".", "createComponent", "callbacks[0]=inline", () => {
  let ran = false;
  web.createComponent(() => {
    ran = true;
    return undefined;
  }, {});
  return ran;
});
await probe("@solidjs/web", ".", "untrack", "callbacks[0]=inline", () => {
  let ran = false;
  web.untrack(() => {
    ran = true;
  });
  return ran;
});

if (!isServer) {
  await probe("@solidjs/web", ".", "getNextElement", "callbacks[0]=inline", () => {
    let ran = false;
    web.getNextElement(() => {
      ran = true;
      return {};
    });
    return ran;
  });
}

await probe(
  "@solidjs/web",
  ".",
  "dynamic",
  `callbacks[0]=${isServer ? "inline" : "tracked"}`,
  () => {
    let runs = 0;
    const Component = web.dynamic(() => {
      runs++;
      return () => undefined;
    });
    if (runs !== (isServer ? 1 : 0)) return false;
    const rendered = Component({});
    if (typeof rendered === "function") rendered();
    return runs === 1;
  },
);

if (isServer) {
  await probe("@solidjs/web", ".", "renderToString", "callbacks[0]=inline", () => {
    let ran = false;
    web.renderToString(() => {
      ran = true;
      return "ok";
    });
    return ran;
  });
  await probe("@solidjs/web", ".", "ssrElement", "callbacks[1]=inline", () => {
    let ran = false;
    web.ssrElement("div", () => {
      ran = true;
      return {};
    }, undefined, false);
    return ran;
  });
  await probe("@solidjs/web", ".", "ssrElement", "callbacks[2]=inline", () => {
    let ran = false;
    web.ssrElement("div", {}, () => {
      ran = true;
      return "child";
    }, false);
    return ran;
  });
}

emit(packages, probes);
