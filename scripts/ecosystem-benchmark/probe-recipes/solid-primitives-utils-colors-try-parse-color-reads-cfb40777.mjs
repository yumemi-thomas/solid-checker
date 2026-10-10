// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `./colors` runtime case of `@kobalte/core@2.0.0-alpha.0`'s closure
// `artifact-case:cfb407779e190a732d4118a19242f5ae4dacc292fd7aba9149a31aba2986abfa`
// (the same module bytes as the `8a0d569b` case, under another importer's closure).
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
// `tryParseColor(value)` is `parseColor` with the throw turned into
// `undefined`. Same reads, same absence of any owned reactive source.
import { tryParseColor } from "@solid-primitives/utils/colors";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  expect("tryParseColor('nope') is undefined", tryParseColor("nope") === undefined);
  expect("tryParseColor('') is undefined", tryParseColor("") === undefined);
  const parsed = tryParseColor("#00ff00");
  expect("tryParseColor('#00ff00') parses", parsed !== undefined && parsed.getChannelValue("green") === 255);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
