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
// NEVER EMITS: `read-operation`. Two string comparisons and an `indexOf` over a module-local array of literals; no argument is dereferenced beyond its own value.
import { isButton } from "@corvu-next/utils";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  // This fork takes `(tagName, type)` as lowercase strings, not an element --
  // the same-named export of `@corvu/utils` does not, so the samples are the
  // package's own and not carried across.
  expect(isButton("button") === true, "a button tag");
  expect(isButton("input", "submit") === true, "an input whose type is button-like");
  expect(isButton("input", "text") === false, "an input whose type is not");
  expect(isButton("input") === false, "an input with no type at all");
  expect(isButton("div") === false, "any other tag");
  expect(isButton("BUTTON") === false, "the comparison is case-sensitive");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
