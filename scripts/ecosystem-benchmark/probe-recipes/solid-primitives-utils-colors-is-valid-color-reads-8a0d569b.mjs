// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `./colors` runtime case
// `artifact-case:8a0d569bf4c932e37be45aead33703aedcde48c3f9e0ab78b00b948af3d4c17b`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/colors/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// It hands the package no `session` and no `harness`.
//
// `isValidColor(value)` is `tryParseColor(value) !== undefined`. A parse of
// the caller's string with the package's regular expressions; no reactive
// source is involved.
import { isValidColor } from "@solid-primitives/utils/colors";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  expect("#fff is valid", isValidColor("#fff") === true);
  expect("rgb(1, 2, 3) is valid", isValidColor("rgb(1, 2, 3)") === true);
  expect("hsl(120, 50%, 50%) is valid", isValidColor("hsl(120, 50%, 50%)") === true);
  expect("'nope' is not valid", isValidColor("nope") === false);
  expect("'#ggg' is not valid", isValidColor("#ggg") === false);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
