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
// `tryOnCleanup` is `isDev ? fn => getOwner() ? onCleanup(fn) : fn :
// onCleanup`,
// where `isDev` is `isClient && !!DEV` and `isClient` is `typeof window !==
// "undefined"`. The probe realm is a Node worker with no `window`, so the
// binding this recipe samples is Solid's own `onCleanup`, which returns its
// argument whether or not an owner is current. Both arms read only Solid's
// current owner -- state `solid-js` owns, not this package -- so neither is a
// read this domain is about.
//
// What it cannot do: reach the `isDev` arm. That arm is unreachable in any
// Node realm the harness can build, and the recipe says so rather than
// pretending to have sampled it.
//
// It hands the package no `session` and no `harness`.
//
// What it does not do: close the domain. The census refuses this candidate on
// its own premises -- the transcript is open with `callSignatureNotUnique`, or
// the form states no reviewed subject root -- and a veto cannot supply a
// premise. This recipe is here because it makes that refusal *visible*: without
// it the candidate is withheld as `no recipe in corpus` and weakened out of the
// plan before its census ever runs. See `docs/precision-backlog.md`, 2026-09-18.
import { tryOnCleanup } from "@solid-primitives/utils";

const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let ran = 0;
  const disposal = () => {
    ran += 1;
  };
  // No owner is current here, so the registration cannot be observed from
  // outside; what is observable is that the callable comes straight back and
  // that the package never ran it.
  expect("tryOnCleanup returns its argument", tryOnCleanup(disposal) === disposal);
  expect("tryOnCleanup did not run the disposal", ran === 0);

  let callerReads = 0;
  const ownedByTheRecipe = {
    get current() {
      callerReads += 1;
      return disposal;
    }
  };
  expect(
    "tryOnCleanup over a caller getter",
    tryOnCleanup(ownedByTheRecipe.current) === disposal
  );
  // The apparatus is live -- the getter fired once, on the recipe's own access
  // in the line above -- and that read is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", callerReads === 1);
  expect("tryOnCleanup still did not run the disposal", ran === 0);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
