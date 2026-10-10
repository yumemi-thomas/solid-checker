// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `./immutable` runtime case
// `artifact-case:b70ad6d1290a49f1c7da26503bd848d3979fbb44e3e0fca19c60ed92c59bae47`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/immutable/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// It hands the package no `session` and no `harness`.
//
// `removeItems(list, ...items)` walks the caller's array once and keeps every
// element not matched by a remaining item, each item matching once. Plain
// index reads of caller-supplied arrays (ADR 0034).
import { removeItems } from "@solid-primitives/utils/immutable";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const list = [1, 2, 3, 2];
  expect("removeItems([1, 2, 3, 2], 2) removes one occurrence", same(removeItems(list, 2), [1, 3, 2]));
  expect("removeItems([1, 2, 3, 2], 2, 2) removes both", same(removeItems(list, 2, 2), [1, 3]));
  expect("removeItems leaves the caller's array alone", same(list, [1, 2, 3, 2]));
  expect("removeItems with nothing to remove", same(removeItems([5], 9), [5]));
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
