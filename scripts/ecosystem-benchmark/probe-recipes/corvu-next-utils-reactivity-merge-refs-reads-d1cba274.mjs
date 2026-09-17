// Hand-authored `reads: []` veto for `@corvu-next/utils@0.1.5`,
// on the published `./reactivity` runtime case
// `artifact-case:d1cba2744ea8e48402d49970137c7982c91259c4418b476381316c7dd5abe5cd`
// (the corvu-next fork's node 1 row certify through; the chunk
// `dist/chunk/ZV6G25TT.js` is byte-identical to `@corvu/utils@0.4.2`'s, whose
// four recipes this one carries).
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
import { access, chain, mergeRefs, some } from "@corvu-next/utils/reactivity";
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
