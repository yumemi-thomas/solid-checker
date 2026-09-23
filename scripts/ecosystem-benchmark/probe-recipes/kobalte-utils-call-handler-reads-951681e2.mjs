// Hand-authored `reads: []` veto for `@kobalte/utils@2.0.0-alpha.0`,
// on the published `.` runtime case
// `artifact-case:951681e29670fb212388347eab8ad87065ad8d59ea60af3e6a5d953cdd22664e`.
//
// Demand-scoped: 112 call sites across the pinned consumer corpus name this
// export, the most expensive single export the corpus leaves open.
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
// `callHandler(event, handler)` is three reads and one invocation: it tests
// `handler`, reads `handler[0]` and `handler[1]` when the handler is a bound
// tuple, and answers `event?.defaultPrevented`. Every one of them is reached
// from a value the caller passed, which is the caller's read under ADR 0034,
// and the export holds no source of its own for a sample to trip.
//
// What it cannot do: say anything about the `callbacks` domain, which is where
// the invocation of the caller's handler belongs and which this artifact case
// leaves open.
import { callHandler } from "@kobalte/utils";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const seen = [];
  const event = { defaultPrevented: false };
  expect("a function handler runs", callHandler(event, e => seen.push(e)) === false);
  expect("it received the caller's event", seen[0] === event);

  const bound = [(bindee, e) => seen.push([bindee, e]), "bindee"];
  expect("a tuple handler runs", callHandler(event, bound) === false);
  expect("the tuple was applied in order", seen[1][0] === "bindee" && seen[1][1] === event);

  expect("no handler is tolerated", callHandler(event, undefined) === false);
  expect("no event answers undefined", callHandler(undefined, undefined) === undefined);
  expect("only the two samples ran", seen.length === 2);

  let callerReads = 0;
  const ownedByTheRecipe = {
    get defaultPrevented() {
      callerReads += 1;
      return true;
    }
  };
  expect("it answers the caller's own accessor", callHandler(ownedByTheRecipe, undefined) === true);
  // The apparatus is live: the getter fired, and it sits on the event object
  // the caller handed over, so the read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", callerReads === 1);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
