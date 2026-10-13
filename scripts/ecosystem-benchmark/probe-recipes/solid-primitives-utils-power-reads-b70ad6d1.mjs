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
// is no trap here for the recipe to count (§ 12 of `docs/package-
// contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`). The
// hazard does the work an observation counter used to. What is left is what a
// mandatory veto is for: sample the export and emit only on contradiction. It
// hands the package no `session` and no `harness`. `power(a, ...b)` folds `a =
// a ** n` over the rest, left to right. It folds the numbers the caller passed
// and owns no source of its own; the caller-owned getter below is read exactly
// once, by the recipe's own access, and that read is the caller's under ADR
// 0034.
//
// What it does not do: close the domain. The census refuses this candidate on
// its own premises -- the transcript is open with `callSignatureNotUnique`, or
// the form states no reviewed subject root -- and a veto cannot supply a
// premise. This recipe is here because it makes that refusal *visible*: without
// it the candidate is withheld as `no recipe in corpus` and weakened out of the
// plan before its census ever runs. See `docs/precision-backlog.md`, 2026-09-18.
import { power } from "@solid-primitives/utils/immutable";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  expect("power(3)", power(3) === 3);
  expect("power(2, 3)", power(2, 3) === 8);
  expect("power folds left to right", power(2, 3, 2) === 64);

  let callerReads = 0;
  const ownedByTheRecipe = {
    get current() {
      callerReads += 1;
      return 2;
    }
  };
  expect("power over a caller getter", Number.isFinite(power(ownedByTheRecipe.current, 1)));
  expect("the caller's getter fired exactly once", callerReads === 1);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
