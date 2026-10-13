// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `./immutable` runtime case
// `artifact-case:b70ad6d1290a49f1c7da26503bd848d3979fbb44e3e0fca19c60ed92c59bae47`.
//
// Demand-scoped: 3 call sites across the pinned consumer corpus name this
// export.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/immutable/index.js` installs no accessor and there
// is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// It hands the package no `session` and no `harness`.
//
// `concat(...a)` builds a fresh array, spreading each argument that is an
// array and pushing every other one whole. It reads only the arguments the
// caller passed (ADR 0034) and writes only the array it made.
//
// What it does not do: close the domain. The census refuses this candidate on
// its own premises -- the transcript is open with `callSignatureNotUnique`, or
// the form states no reviewed subject root -- and a veto cannot supply a
// premise. This recipe is here because it makes that refusal *visible*: without
// it the candidate is withheld as `no recipe in corpus` and weakened out of the
// plan before its census ever runs. See `docs/precision-backlog.md`, 2026-09-18.
import { concat } from "@solid-primitives/utils/immutable";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const list = [1, 2];
  expect("concat spreads arrays", same(concat(list, [3]), [1, 2, 3]));
  expect("concat pushes non-arrays whole", same(concat(list, 3), [1, 2, 3]));
  expect("concat mixes both", same(concat(1, [2, 3], 4), [1, 2, 3, 4]));
  expect("concat of nothing", same(concat(), []));
  expect("concat copies", concat(list) !== list);
  expect("concat leaves the caller's array alone", same(list, [1, 2]));
  expect("concat does not flatten twice", same(concat([[1]], 2), [[1], 2]));
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
