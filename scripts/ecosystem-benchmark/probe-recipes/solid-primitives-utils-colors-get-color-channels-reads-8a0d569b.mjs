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
// `getColorChannels(space)` switches on a string and returns the static
// channel list of one of the package's colour classes. A read of a static
// property the package defined; not a reactive source.
import { getColorChannels } from "@solid-primitives/utils/colors";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const rgb = getColorChannels("rgb");
  expect("getColorChannels('rgb') names red, green and blue", Array.isArray(rgb) && rgb.includes("red") && rgb.includes("green") && rgb.includes("blue"));
  const hsl = getColorChannels("hsl");
  expect("getColorChannels('hsl') names hue, saturation and lightness", hsl.includes("hue") && hsl.includes("saturation") && hsl.includes("lightness"));
  expect("getColorChannels('hsb') names brightness", getColorChannels("hsb").includes("brightness"));
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
