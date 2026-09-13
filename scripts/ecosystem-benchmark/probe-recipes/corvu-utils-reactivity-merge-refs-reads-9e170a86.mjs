// Hand-authored `reads: []` veto for `@corvu/utils@0.3.2`,
// on the published `./reactivity` runtime case
// `artifact-case:9e170a86c50f3bda641662e7bce6a8b846db4acb0d288dce5fd679518ed3d125`
// (the 0.3.2 node three rows certify through; the same four bodies as 0.4.2).
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/reactivity/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// NEVER EMITS: `read-operation`. A `chain` over the supplied refs; each ref receives the element once.
import { access, chain, mergeRefs, some } from "@corvu/utils/reactivity";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const got = [];
  const ref = mergeRefs((el) => got.push(el), null, (el) => got.push(el));
  const element = {};
  ref(element);
  expect(got.length === 2 && got[0] === element && got[1] === element, "each ref receives the element");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
