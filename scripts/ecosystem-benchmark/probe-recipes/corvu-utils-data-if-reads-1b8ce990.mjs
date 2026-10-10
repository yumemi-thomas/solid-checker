// Hand-authored `reads: []` veto for `@corvu/utils@0.4.2`,
// on the published `.` runtime case
// `artifact-case:1b8ce990fb0ee5829fe76cf23d32666c21e6d9e81ea6bb298a3ae675d6eceb27`.
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
// NEVER EMITS: `read-operation`. A pure conditional over its argument: the empty string for a truthy condition, undefined otherwise.
import { dataIf, isButton, isFunction } from "@corvu/utils";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  expect(dataIf(true) === "", "truthy gives the empty data attribute");
  expect(dataIf(false) === undefined, "falsy gives undefined");
  expect(dataIf(0) === undefined && dataIf("x") === "", "coerces like a condition");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
