// Hand-authored `reads: []` veto for `@solid-primitives/trigger@3.0.0-next.2`,
// on the published `.` runtime case
// `artifact-case:8c695fdb30fd2222bb83c63763f8e1d3a4391aa5fa73b6e075efbe8d6dbcc75a`.
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
// `new TriggerCache(mapConstructor = Map)` runs one statement:
// `this.#map = new mapConstructor()`. Constructing reads the caller's
// constructor argument and nothing else -- the signals live in `track`, which
// is a method call at a later event and not this domain's business.
import { TriggerCache } from "@solid-primitives/trigger";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const cache = new TriggerCache();
  expect("it answers an instance", cache instanceof TriggerCache);
  expect("it exposes track", typeof cache.track === "function");
  expect("it exposes dirty", typeof cache.dirty === "function");
  expect("it exposes dirtyAll", typeof cache.dirtyAll === "function");
  expect("a WeakMap cache constructs too", new TriggerCache(WeakMap) instanceof TriggerCache);

  let constructorReads = 0;
  const ownedByTheRecipe = {
    get current() {
      constructorReads += 1;
      return Map;
    }
  };
  expect(
    "TriggerCache over a caller getter",
    new TriggerCache(ownedByTheRecipe.current) instanceof TriggerCache
  );
  // The apparatus is live -- the getter fired once, on the recipe's own access
  // in the line above -- and that read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", constructorReads === 1);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
