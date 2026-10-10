// Hand-authored `reads: []` veto for `@kobalte/utils@2.0.0-alpha.0`,
// on the published `.` runtime case
// `artifact-case:ba7a1b6e62c62494192a2955d54f23af05d662db86d9f95af8ba0908213be109`.
//
// Demand-scoped: 48 call sites across the pinned consumer corpus name this
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
// `composeEventHandlers(handlers)` reads nothing in its own frame: it captures
// the caller's list and returns a closure that iterates it later. The samples
// assert exactly that -- constructing the handler runs none of them -- because
// that is what would have to change for this domain to be wrong. The iteration
// itself happens in the returned closure, a different frame at a different
// event.
import { composeEventHandlers } from "@kobalte/utils";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let ran = 0;
  const handlers = [() => { ran += 1; }, () => { ran += 1; }];
  const composed = composeEventHandlers(handlers);
  expect("composeEventHandlers answers a function", typeof composed === "function");
  expect("constructing ran no handler", ran === 0);
  expect("an empty list still answers a function", typeof composeEventHandlers([]) === "function");

  let listReads = 0;
  const ownedByTheRecipe = {
    get current() {
      listReads += 1;
      return handlers;
    }
  };
  expect(
    "composeEventHandlers over a caller getter",
    typeof composeEventHandlers(ownedByTheRecipe.current) === "function"
  );
  // The apparatus is live -- the getter fired once, on the recipe's own access
  // in the line above -- and that read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", listReads === 1);
  expect("still no handler ran", ran === 0);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
