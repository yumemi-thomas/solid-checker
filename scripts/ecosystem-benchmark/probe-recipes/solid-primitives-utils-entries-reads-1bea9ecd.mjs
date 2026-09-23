// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `.` runtime case
// `artifact-case:1bea9ecdd99dcdbedc17ea6efb689911ddcefe25cbd7a1c13424967dec04dab2`.
//
// Demand-scoped: 37 call sites across the pinned consumer corpus name this
// export, and its summary stays degenerate while the domain is open.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
//
// `entries` is `Object.entries` re-exported under another name -- the module
// binds it at load and never wraps it. Every value it reads is reached from
// the object the caller handed it, which is the caller's read under ADR 0034,
// so the samples below assert the identity and the enumeration rather than
// counting a trap the closure does not own. The last sample proves the
// apparatus is live: a caller-owned getter fires, and is deliberately not
// emitted.
//
// It hands the package no `session` and no `harness`.
//
// What it does not do: close the domain. The census refuses this candidate on
// its own premises -- the transcript is open with `callSignatureNotUnique`, or
// the form states no reviewed subject root -- and a veto cannot supply a
// premise. This recipe is here because it makes that refusal *visible*: without
// it the candidate is withheld as `no recipe in corpus` and weakened out of the
// plan before its census ever runs. See `docs/precision-backlog.md`, 2026-09-18.
import { entries } from "@solid-primitives/utils";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  expect("entries is Object.entries", entries === Object.entries);
  expect("entries({})", same(entries({}), []));
  expect("entries({ a: 1, b: 2 })", same(entries({ a: 1, b: 2 }), [["a", 1], ["b", 2]]));
  expect("entries enumerates own keys only", same(entries(Object.create({ inherited: 1 })), []));

  let callerReads = 0;
  const ownedByTheRecipe = {
    get current() {
      callerReads += 1;
      return "read";
    }
  };
  expect("entries over a caller getter", same(entries(ownedByTheRecipe), [["current", "read"]]));
  // The apparatus is live -- the getter is a real trap and it fired -- and the
  // read is the caller's under ADR 0034, so nothing is emitted for it.
  expect("the caller's getter fired exactly once", callerReads === 1);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
