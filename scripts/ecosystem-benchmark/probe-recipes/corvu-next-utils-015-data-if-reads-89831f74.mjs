// Hand-authored `reads: []` veto for `@corvu-next/utils@0.1.5`,
// on the published `.` runtime case
// `artifact-case:89831f74dfb8be8f38a3becc762f3e96d7666e442c43bed9fd32140097b641fd`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that the module installs no accessor and there is no trap here
// for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// What is left is what a mandatory veto is for: sample the export and emit only
// on contradiction.
//
// NEVER EMITS: `read-operation`. One conditional over the caller's own argument, returning a literal or `undefined`.
import { dataIf } from "@corvu-next/utils";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  expect(dataIf(true) === "", "a truthy condition is the empty attribute value");
  expect(dataIf(false) === undefined, "a falsy condition is absent");
  expect(dataIf(1) === "" && dataIf(0) === undefined, "the test is truthiness, not identity");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
