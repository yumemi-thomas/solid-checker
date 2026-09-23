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
// `defaultEquals` is `Object.is.bind(Object)`. It compares the two values the
// caller handed it by identity and dereferences neither, so the caller-owned
// getter below must not fire.
//
// It hands the package no `session` and no `harness`.
//
// What it does not do: close the domain. The census refuses this candidate on
// its own premises -- the transcript is open with `callSignatureNotUnique`, or
// the form states no reviewed subject root -- and a veto cannot supply a
// premise. This recipe is here because it makes that refusal *visible*: without
// it the candidate is withheld as `no recipe in corpus` and weakened out of the
// plan before its census ever runs. See `docs/precision-backlog.md`, 2026-09-18.
import { defaultEquals } from "@solid-primitives/utils";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const reference = {};
  expect("defaultEquals(1, 1)", defaultEquals(1, 1) === true);
  expect("defaultEquals(1, 2)", defaultEquals(1, 2) === false);
  expect("defaultEquals(NaN, NaN)", defaultEquals(NaN, NaN) === true);
  expect("defaultEquals(0, -0)", defaultEquals(0, -0) === false);
  expect("defaultEquals is identity, not structural", defaultEquals({}, {}) === false);
  expect("defaultEquals on one reference", defaultEquals(reference, reference) === true);

  let callerReads = 0;
  const ownedByTheRecipe = {
    get current() {
      callerReads += 1;
      return "read";
    }
  };
  expect(
    "defaultEquals over a caller getter",
    defaultEquals(ownedByTheRecipe, ownedByTheRecipe) === true
  );
  expect("defaultEquals never dereferenced it", callerReads === 0);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
