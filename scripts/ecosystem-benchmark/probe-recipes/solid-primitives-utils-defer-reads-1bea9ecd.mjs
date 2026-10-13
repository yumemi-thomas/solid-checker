// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `.` runtime case
// `artifact-case:1bea9ecdd99dcdbedc17ea6efb689911ddcefe25cbd7a1c13424967dec04dab2`.
//
// Not named by the pinned consumer demand; written because the scaffold pass
// showed the census can decide this candidate and one recipe on this
// dependency node closes the entry in every row that depends on it.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
//
// `defer(deps, fn, initialValue)` does one thing in its own frame: `isArray =
// Array.isArray(deps)`, then it returns an accessor. Nothing is read at call
// time -- above all not `deps`, which the returned accessor calls later, in a
// frame of its own and on a value the caller supplied (ADR 0034). The samples
// therefore assert that constructing the accessor invokes neither `deps` nor
// `fn`, which is what would have to change for this domain to be wrong.
//
// What it cannot do: say anything about the returned accessor's own reads.
// Those are a different frame at a different event, and this domain is the
// call path of `defer` itself.
//
// It hands the package no `session` and no `harness`.
//
// What it does not do: close the domain. The census refuses this candidate on
// its own premises -- the transcript is open with `callSignatureNotUnique`, or
// the form states no reviewed subject root -- and a veto cannot supply a
// premise. This recipe is here because it makes that refusal *visible*: without
// it the candidate is withheld as `no recipe in corpus` and weakened out of the
// plan before its census ever runs. See `docs/precision-backlog.md`, 2026-09-18.
import { defer } from "@solid-primitives/utils";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let dependencyCalls = 0;
  let transformCalls = 0;
  const dependency = () => {
    dependencyCalls += 1;
    return "value";
  };
  const transform = () => {
    transformCalls += 1;
    return "transformed";
  };

  const single = defer(dependency, transform, "initial");
  expect("defer answers a function", typeof single === "function");
  expect("defer did not call deps", dependencyCalls === 0);
  expect("defer did not call fn", transformCalls === 0);

  const many = defer([dependency, dependency], transform, "initial");
  expect("defer answers a function for an array", typeof many === "function");
  expect("the array form did not call deps either", dependencyCalls === 0);
  expect("the array form did not call fn either", transformCalls === 0);

  let callerReads = 0;
  const ownedByTheRecipe = {
    get current() {
      callerReads += 1;
      return dependency;
    }
  };
  expect(
    "defer over a caller getter",
    typeof defer(ownedByTheRecipe.current, transform) === "function"
  );
  // The apparatus is live -- the getter fired once, on the recipe's own access
  // in the line above -- and that read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", callerReads === 1);
  expect("no construction called deps", dependencyCalls === 0);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
