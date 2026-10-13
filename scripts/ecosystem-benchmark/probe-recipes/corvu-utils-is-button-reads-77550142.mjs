// Hand-authored `reads: []` veto for `@corvu/utils@0.4.2`,
// on the published `.` runtime case
// `artifact-case:77550142dae6654efbf27c749c56d8305f601aaa5d54150e3d2aed049c5492f6`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// NEVER EMITS: `read-operation`. A pure comparison of a tag name and an input type against a module-level list of literals.
import { dataIf, isButton, isFunction } from "@corvu/utils";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  expect(isButton("button") === true, "button tag");
  expect(isButton("input", "submit") === true, "submit input");
  expect(isButton("input", "text") === false, "text input");
  expect(isButton("div") === false, "other tag");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
