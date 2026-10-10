// Hand-authored `reads: []` veto for `@kobalte/utils@2.0.0-alpha.0`,
// on the published `.` runtime case
// `artifact-case:a442b2aeb8cf8e1885b545bce84e7c46b4a2be2f3dd79ff5fcf6835d7b08e1b6`.
//
// Demand-scoped: 6 call sites across the pinned consumer corpus name this
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
// `isPointInPolygon(point, polygon)` destructures the caller's point and walks
// the caller's vertex list, destructuring each pair. Every read is of a value
// the caller passed (ADR 0034) and it allocates nothing that outlives the call.
import { isPointInPolygon } from "@kobalte/utils";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const square = [[0, 0], [10, 0], [10, 10], [0, 10]];
  expect("a point inside", isPointInPolygon([5, 5], square) === true);
  expect("a point outside", isPointInPolygon([50, 50], square) === false);
  expect("a point left of the square", isPointInPolygon([-1, 5], square) === false);
  expect("an empty polygon contains nothing", isPointInPolygon([0, 0], []) === false);

  let vertexReads = 0;
  const ownedByTheRecipe = {
    get current() {
      vertexReads += 1;
      return square;
    }
  };
  expect(
    "isPointInPolygon over a caller getter",
    isPointInPolygon([5, 5], ownedByTheRecipe.current) === true
  );
  // The apparatus is live -- the getter fired once, on the recipe's own access
  // in the line above -- and that read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", vertexReads === 1);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
