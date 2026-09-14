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
// `new Store(v, f)` seeds an atom, binds three of its own methods, and — only when a factory is passed — hands the fresh instance to it. The factory is caller-supplied code under ADR 0034, so what it reads is the caller's; the samples pass one to pin that it runs exactly once per construction.
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
import { Store } from "@tanstack/store";

export async function runProbeSession(_session, harness) {
  const samples = 3;
  let observed = 0;
  let factoryRuns = 0;
  let factoryReads = 0;
  const ownedByTheRecipe = {
    get current() {
      factoryReads += 1;
      return "read";
    }
  };
  for (let sample = 0; sample < samples; sample += 1) {
    const seed = { label: "seed", get trap() { observed += 1; return 1; } };
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
      const built = new Store(seed, (store) => {
        factoryRuns += 1;
        if (store === undefined) {
          throw new Error("the actions factory was handed no store");
        }
        // A read of the recipe's own object from inside caller-supplied code.
        // It is the caller's under ADR 0034 and is deliberately not emitted.
        return { owned: ownedByTheRecipe.current };
      });
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
    if (typeof built?.get !== "function" || typeof built?.subscribe !== "function") {
      throw new Error("Store did not construct an instance with get and subscribe");
    }
    if (built.get() !== seed) {
      throw new Error("Store did not seed its atom with the value it was constructed from");
    }
  }
  await Promise.resolve();
  if (factoryRuns !== samples) {
    throw new Error(`the caller's actions factory ran ${factoryRuns} time(s), expected ${samples}`);
  }
  // The apparatus proof for this export: the factory is caller-supplied code
  // that ran inside the construction, and the getter it read registered every
  // time. A run where this counter stayed at zero is a run where an owned read
  // could not have been seen either.
  if (factoryReads !== samples) {
    throw new Error("the recipe's own getter did not register inside the actions factory");
  }
  if (observed !== 0) {
    throw new Error(`construction read the caller's own property ${observed} time(s)`);
  }
}
