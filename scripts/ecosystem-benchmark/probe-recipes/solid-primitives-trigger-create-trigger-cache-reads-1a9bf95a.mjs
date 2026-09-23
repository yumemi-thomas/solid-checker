// Hand-authored `reads: []` veto for `@solid-primitives/trigger@3.0.0-next.2`,
// on the published `.` runtime case
// `artifact-case:1a9bf95ab59ef488893fe527406cc43b315458daef5b338aa20243eb6b971b54`.
//
// Demand-scoped: 3 call sites across the pinned consumer corpus name this
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
// `createTriggerCache(mapConstructor = Map)` builds a `TriggerCache` and
// answers three of its methods bound to it. The construction reads the caller's
// constructor argument; the bindings read the instance the call just made.
// Neither is a read of a reactive source this package owns -- those are created
// in `track`, at a later event.
import { createTriggerCache } from "@solid-primitives/trigger";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const answered = createTriggerCache();
  expect("it answers a triple", Array.isArray(answered) && answered.length === 3);
  expect("all three are callable", answered.every(entry => typeof entry === "function"));
  expect("a WeakMap cache answers a triple too", createTriggerCache(WeakMap).length === 3);

  let constructorReads = 0;
  const ownedByTheRecipe = {
    get current() {
      constructorReads += 1;
      return Map;
    }
  };
  expect("createTriggerCache over a caller getter", createTriggerCache(ownedByTheRecipe.current).length === 3);
  // The apparatus is live -- the getter fired once, on the recipe's own access
  // in the line above -- and that read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", constructorReads === 1);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
