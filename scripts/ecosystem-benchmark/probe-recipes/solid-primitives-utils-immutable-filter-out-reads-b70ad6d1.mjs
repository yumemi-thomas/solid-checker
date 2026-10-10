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
// `filterOut(list, item)` is `list.filter(i => i !== item)` with the count of
// dropped items written on the result as `removed`. It reads only the array
// the caller passed (ADR 0034) and the caller's list is left as it was.
import { filterOut } from "@solid-primitives/utils/immutable";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const list = [1, 2, 1, 3];
  const answered = filterOut(list, 1);
  expect("filterOut([1, 2, 1, 3], 1)", same(answered, [2, 3]) && answered.removed === 2);
  expect("filterOut leaves the caller's array alone", same(list, [1, 2, 1, 3]));
  const none = filterOut([4, 5], 9);
  expect("filterOut with nothing to drop", same(none, [4, 5]) && none.removed === 0);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
