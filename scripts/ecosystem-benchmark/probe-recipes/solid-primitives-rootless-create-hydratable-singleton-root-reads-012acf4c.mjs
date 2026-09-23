// Hand-authored `reads: []` veto for `@solid-primitives/rootless@2.0.0-next.2`,
// on the published `.` runtime case
// `artifact-case:012acf4c9373c95ab5ed8063d60ed63efd71d8855bbad0d845ce8c39feda0c03`.
//
// Demand-scoped: 27 call sites across the pinned consumer corpus name this
// export -- the largest single demand this package leaves open.
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
// `createHydratableSingletonRoot(factory)` does two things in its own frame:
// it asks Solid for the current owner and it builds a singleton closure over
// the caller's factory. Both reads belong to `solid-js` rather than to this
// package, and no root is created until the returned accessor is called -- a
// different frame at a different event, which this domain is not about.
//
// The samples assert that construction runs the caller's factory zero times,
// which is what would have to change for a read at the call event to appear.
import { createHydratableSingletonRoot } from "@solid-primitives/rootless";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let factoryCalls = 0;
  const factory = () => {
    factoryCalls += 1;
    return "value";
  };
  const useValue = createHydratableSingletonRoot(factory);
  expect("it answers an accessor", typeof useValue === "function");
  expect("construction ran no factory", factoryCalls === 0);

  const second = createHydratableSingletonRoot(factory);
  expect("a second construction answers its own accessor", second !== useValue);
  expect("and still ran no factory", factoryCalls === 0);

  let callerReads = 0;
  const ownedByTheRecipe = {
    get current() {
      callerReads += 1;
      return factory;
    }
  };
  expect(
    "createHydratableSingletonRoot over a caller getter",
    typeof createHydratableSingletonRoot(ownedByTheRecipe.current) === "function"
  );
  // The apparatus is live -- the getter fired once, on the recipe's own access
  // in the line above -- and that read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", callerReads === 1);
  expect("no construction ran the factory", factoryCalls === 0);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
