// Hand-authored `reads: []` veto for `@kobalte/utils@2.0.0-alpha.0`,
// on the published `.` runtime case
// `artifact-case:0d98b1255315f2e9b474059c67b9b216cccbc4614e5cf3f89cc78b710a93a7da`.
//
// Demand-scoped: 16 call sites across the pinned consumer corpus name this
// export.
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
// It hands the package no `session` and no `harness`.
//
// `clamp(value, min = -Infinity, max = +Infinity)` is
// `Math.min(Math.max(value, min), max)`. It folds the numbers the caller passed
// through two reviewed default-library members and owns no source of its own.
import { clamp } from "@kobalte/utils";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  expect("clamp inside the range", clamp(5, 0, 10) === 5);
  expect("clamp below the floor", clamp(-1, 0, 10) === 0);
  expect("clamp above the ceiling", clamp(11, 0, 10) === 10);
  expect("clamp with no bounds is identity", clamp(42) === 42);
  expect("clamp with only a floor", clamp(-1, 0) === 0);
  expect("clamp lets the ceiling win a crossed range", clamp(5, 10, 0) === 0);

  let callerReads = 0;
  const ownedByTheRecipe = {
    get current() {
      callerReads += 1;
      return 5;
    }
  };
  expect("clamp over a caller getter", clamp(ownedByTheRecipe.current, 0, 10) === 5);
  // The apparatus is live -- the getter fired once, on the recipe's own access
  // in the line above -- and that read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", callerReads === 1);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
