// Hand-authored `reads: []` veto for `@corvu-next/utils@0.1.5`,
// on the published `./create/keyedContext` runtime case
// `artifact-case:9d31fed16de4a03416f4534ddcdc1fa4b478c5ed00f27caa6a69f1282f14d607`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that the module installs no accessor and there is no trap here
// for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// What is left is what a mandatory veto is for: sample the export and emit only
// on contradiction.
//
// NEVER EMITS: `read-operation`. A lookup in the package's own registry returns the context object itself; the context's *value* is never observed here, and the registry is not a reactive source.
import { createRoot } from "solid-js";
import { createKeyedContext, getKeyedContext } from "@corvu-next/utils/create/keyedContext";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  createRoot((dispose) => {
    const context = createKeyedContext("recipe:get", 3);
    expect(getKeyedContext("recipe:get") === context, "a registered key returns its context");
    expect(getKeyedContext("recipe:absent") === undefined, "an unregistered key returns undefined");
    dispose();
  });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
