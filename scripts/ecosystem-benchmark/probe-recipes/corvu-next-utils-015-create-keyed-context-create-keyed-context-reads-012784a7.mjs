// Hand-authored `reads: []` veto for `@corvu-next/utils@0.1.5`,
// on the published `./create/keyedContext` runtime case
// `artifact-case:012784a7b42a2fbb4e8898f17fa7695e7074946e64f7c1fb147edf816379da23`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that the module installs no accessor and there is no trap here
// for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// What is left is what a mandatory veto is for: sample the export and emit only
// on contradiction.
//
// NEVER EMITS: `read-operation`. The export builds a context and registers it under the caller's key; building a source is not reading one.
import { createRoot } from "solid-js";
import { createKeyedContext, getKeyedContext } from "@corvu-next/utils/create/keyedContext";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  createRoot((dispose) => {
    const context = createKeyedContext("recipe:create", 7);
    expect(typeof context === "function", "a context is returned");
    expect(getKeyedContext("recipe:create") === context, "it is registered under the key");
    dispose();
  });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
