// Hand-authored `reads: []` veto for `@solid-primitives/rootless@2.0.0-next.2`,
// on the published `.` runtime case
// `artifact-case:012acf4c9373c95ab5ed8063d60ed63efd71d8855bbad0d845ce8c39feda0c03`.
//
// Demand-scoped: 7 call sites across the pinned consumer corpus name this
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
// `createSingletonRoot(factory, detachedOwner = getOwner())` reads Solid's
// current owner for its default argument and returns a closure over three
// locals it just declared. Nothing this package owns is read, and the root is
// not created until the returned accessor runs -- a different frame at a
// different event.
import { createSingletonRoot } from "@solid-primitives/rootless";

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
  const useValue = createSingletonRoot(factory);
  expect("it answers an accessor", typeof useValue === "function");
  expect("construction ran no factory", factoryCalls === 0);
  expect("an explicit owner is accepted", typeof createSingletonRoot(factory, null) === "function");
  expect("and still ran no factory", factoryCalls === 0);

  let callerReads = 0;
  const ownedByTheRecipe = {
    get current() {
      callerReads += 1;
      return factory;
    }
  };
  expect(
    "createSingletonRoot over a caller getter",
    typeof createSingletonRoot(ownedByTheRecipe.current, null) === "function"
  );
  // The apparatus is live -- the getter fired once, on the recipe's own access
  // in the line above -- and that read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", callerReads === 1);
  expect("no construction ran the factory", factoryCalls === 0);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
