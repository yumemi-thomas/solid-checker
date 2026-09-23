// Hand-authored `reads: []` veto for `@solid-primitives/rootless@2.0.0-next.2`,
// on the published `.` runtime case
// `artifact-case:dcf64d37fe665149df303a63a3166d314c837d78e91ad35f1bb4a6a2dcaa6ac1`.
//
// Not named by the pinned consumer demand; written because it is the
// deprecated alias of `createSingletonRoot` -- the same binding, so the same
// facts -- and one recipe on this dependency node closes the entry in every
// row that depends on it.
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
// `createSharedRoot` *is* `createSingletonRoot`: the module binds one to the
// other and exports both names. The samples assert that identity first, so a
// release that made them diverge fails the recipe instead of inheriting a
// claim written about the other function.
import { createSharedRoot, createSingletonRoot } from "@solid-primitives/rootless";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  expect("createSharedRoot is createSingletonRoot", createSharedRoot === createSingletonRoot);
  let factoryCalls = 0;
  const factory = () => {
    factoryCalls += 1;
    return "value";
  };
  expect("it answers an accessor", typeof createSharedRoot(factory, null) === "function");
  expect("construction ran no factory", factoryCalls === 0);

  let callerReads = 0;
  const ownedByTheRecipe = {
    get current() {
      callerReads += 1;
      return factory;
    }
  };
  expect(
    "createSharedRoot over a caller getter",
    typeof createSharedRoot(ownedByTheRecipe.current, null) === "function"
  );
  // The apparatus is live -- the getter fired once, on the recipe's own access
  // in the line above -- and that read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", callerReads === 1);
  expect("no construction ran the factory", factoryCalls === 0);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
