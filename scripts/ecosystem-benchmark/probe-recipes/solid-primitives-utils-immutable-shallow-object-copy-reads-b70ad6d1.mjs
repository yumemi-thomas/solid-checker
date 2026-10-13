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
// `shallowObjectCopy(object)` is `Object.assign({}, object)`: one shallow read
// of the caller's own enumerable properties (ADR 0034).
import { shallowObjectCopy } from "@solid-primitives/utils/immutable";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const object = { a: 1, nested: { b: 2 } };
  const copied = shallowObjectCopy(object);
  expect("shallowObjectCopy is an equal new object", same(copied, object) && copied !== object);
  expect("shallowObjectCopy shares nested values", copied.nested === object.nested);
  expect("shallowObjectCopy({}) is {}", same(shallowObjectCopy({}), {}));
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
