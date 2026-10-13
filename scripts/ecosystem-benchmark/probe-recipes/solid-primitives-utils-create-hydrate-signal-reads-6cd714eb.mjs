// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`, on the
// published `.` runtime case `artifact-case:6cd714eb04fb05bfa0d7b88061837c8b505d6d91829178ccd8730a40be329b6d`.
//
// ADR 0104 is what made this candidate decidable. Until protocol 58 the
// implementation census refused `createHydrateSignal` outright: its only
// uncensused form is `sharedConfig.hydrating`, whose receiver is imported from
// `solid-js`, and an imported binding rooted nothing. The producer now states
// the imported pair and the certifier's reviewed table answers that
// `solid-js`'s `sharedConfig` is a data object. That closes the census; this
// recipe is the mandatory veto the closure still needs, because an empty
// `reads` enumeration takes no synthesized one -- its contradiction is a read
// of a source the export owns, which no generated module can instrument.
//
// What the export does: on the server it answers `createSignal(serverValue,
// options)` and stops. On the client it reads `sharedConfig.hydrating`, and
// either seeds a signal and defers `update()` through `onSettled`, or calls
// `update()` immediately and seeds from its result. Every value it reads is
// the caller's or the dependency's; it owns no reactive source and observes
// none.
//
// The recipe's `equals` getter is the observation apparatus, and the samples
// assert it fired: the export passes the caller's options object into
// `createSignal`, which copies its own enumerable properties, so a run in
// which nothing read it is a run in which this recipe could not have seen an
// owned read either. That read is the **caller's** under ADR 0034 and is
// deliberately not emitted.
//
// NEVER EMITS: `read-operation`. The export observes no reactive source of its
// own on any branch.
//
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { createHydrateSignal, isServer } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let equalsReads = 0;
  let updateCalls = 0;
  for (let sample = 0; sample < 3; sample += 1) {
    const options = {
      get equals() {
        equalsReads += 1;
        return false;
      }
    };
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = createHydrateSignal("from-the-server", () => {
      updateCalls += 1;
      return "from-the-client";
    }, options);
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
    if (!Array.isArray(answered) || answered.length !== 2 ||
      typeof answered[0] !== "function" || typeof answered[1] !== "function") {
      throw new Error("createHydrateSignal did not answer a [getter, setter] pair");
    }
  }
  await Promise.resolve();
  if (equalsReads === 0) {
    throw new Error("nothing read the caller's options: this recipe could not have observed an owned read either");
  }
  // On the client the export calls `update()` once per sample and seeds from
  // its result; on the server it never reaches it. Either is correct, and
  // pinning which one ran keeps a package edit that changes the branch from
  // passing this gate silently.
  const expected = isServer ? 0 : 3;
  if (updateCalls !== expected) {
    throw new Error(`the caller's update ran ${updateCalls} time(s), expected ${expected}`);
  }
}
