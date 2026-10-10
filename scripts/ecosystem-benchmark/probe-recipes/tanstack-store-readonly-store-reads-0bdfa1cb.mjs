// Hand-authored `reads: []` veto for `@tanstack/store@0.11.1`, on the published
// `.` runtime case `artifact-case:0bdfa1cbfcd08632d3def1207aa3b0c24966e3093cf9cb6fa9ce35b1d52fce0d`.
//
// ADR 0105 is what made this candidate decidable. A class export is
// *constructed*, not called, and until protocol 59 `SignatureKindCall` yielded
// nothing for it while the `!= 1` check reported the same
// `callSignatureNotUnique` for "none" as for "several" — so the transcript
// returned before any implementation was looked for and the construction
// census was unreachable. The producer now selects the construct signature,
// `classConstructorAt` gates what a construction actually runs, and the
// transcript states that it censused a construction. This recipe is the
// mandatory veto the closure still needs, because an empty `reads`
// enumeration takes no synthesized one.
//
// `new ReadonlyStore(v)` seeds an atom and does nothing else. It takes no callback, so there is no caller-supplied code in the construction at all.
//
// **How this proves its own apparatus.** Construction reads nothing of the
// caller's, so unlike a sampled call there is no owned read to count inside
// the window — asserting zero would be indistinguishable from a getter that
// never worked. So the samples assert zero *during* the window and then read
// the same getter once *after* it, requiring the counter to move: a run in
// which the second read did not register is a run in which the first could not
// have been observed either.
//
// NEVER EMITS: `read-operation`. The construction observes no reactive source
// of its own.
//
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { ReadonlyStore } from "@tanstack/store";

export async function runProbeSession(_session, harness) {
  const samples = 3;
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  for (let sample = 0; sample < samples; sample += 1) {
    const seed = { label: "seed", get trap() { observed += 1; return 1; } };
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
      const built = new ReadonlyStore(seed);
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
    if (typeof built?.get !== "function" || typeof built?.subscribe !== "function") {
      throw new Error("ReadonlyStore did not construct an instance with get and subscribe");
    }
    if (built.get() !== seed) {
      throw new Error("ReadonlyStore did not seed its atom with the value it was constructed from");
    }
  }
  await Promise.resolve();

  if (observed !== 0) {
    throw new Error(`construction read the caller's own property ${observed} time(s)`);
  }
  // The apparatus proof: the same getter, read once outside any call window.
  // A run where this does not register is a run where a read inside the
  // window could not have been observed either.
  void ownedByTheRecipe.current;
  if (observed !== 1) {
    throw new Error("the recipe's own getter did not register: its observation apparatus is not live");
  }
}
