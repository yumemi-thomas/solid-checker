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
// `withCopy(source, mutator)` dispatches on `Array.isArray` to the array or
// object copy and runs the caller's mutator over the copy. Every read is of
// the caller's value or inside the caller's mutator (ADR 0034).
import { withCopy } from "@solid-primitives/utils/immutable";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const object = { a: 1 };
  let ran = 0;
  const answeredObject = withCopy(object, copy => {
    ran += 1;
    copy.b = 2;
  });
  expect("withCopy(object) copies then mutates the copy", ran === 1 && same(answeredObject, { a: 1, b: 2 }) && same(object, { a: 1 }));
  const array = [1];
  const answeredArray = withCopy(array, copy => copy.push(2));
  expect("withCopy(array) copies then mutates the copy", same(answeredArray, [1, 2]) && same(array, [1]));
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
