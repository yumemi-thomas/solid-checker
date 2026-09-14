// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the
// published `.` runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// ADR 0106 is what made this candidate decidable. `createMicrotask` returns
// `(...a) => { (args = a), calls++; queueMicrotask(() => --calls === 0 &&
// fn(...args)); }`, and until that ADR the `fn(...args)` spread was an
// uncensused iteration-protocol form: `args` is an *alias* of the rest
// parameter rather than the rest binding itself, so the reasoning that a rest
// array is one the engine built with ArrayCreate — and therefore spreads
// through `Array.prototype[Symbol.iterator]` and nothing else — stopped one
// hop short. This recipe is the mandatory veto the closure still needs,
// because an empty `reads` enumeration takes no synthesized one.
//
// What the export does: the call itself only allocates two bindings and
// registers an `onCleanup`. Everything else happens in the returned scheduler
// and in the microtask it queues, and what those read — the caller's `fn`, the
// caller's arguments — is the caller's under ADR 0034.
//
// The samples drive the whole path anyway: they call the scheduler, let the
// microtask drain, and assert the caller's `fn` saw exactly the arguments that
// were passed. A run where the spread had reached a different iterator would
// deliver different arguments, and this is what would catch it.
//
// NEVER EMITS: `read-operation`. The export observes no reactive source of its
// own on any path.
//
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { createMicrotask } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  const delivered = [];
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const schedule = createMicrotask((...received) => {
    delivered.push(received);
  });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
  if (typeof schedule !== "function") {
    throw new Error("createMicrotask did not answer a scheduler");
  }
  // Three arguments, one of them the recipe's own object, so the spread has
  // something with identity to carry through.
  schedule("first", ownedByTheRecipe, 3);
  await Promise.resolve();
  await Promise.resolve();
  if (delivered.length !== 1) {
    throw new Error(`the caller's callback ran ${delivered.length} time(s), expected 1`);
  }
  const [args] = delivered;
  if (args.length !== 3 || args[0] !== "first" || args[1] !== ownedByTheRecipe || args[2] !== 3) {
    throw new Error("the spread did not deliver the scheduler's own arguments unchanged");
  }
  if (observed !== 0) {
    throw new Error(`the export read the recipe's own property ${observed} time(s)`);
  }
  // The apparatus proof: the same getter, read once outside any call window.
  void ownedByTheRecipe.current;
  if (observed !== 1) {
    throw new Error("the recipe's own getter did not register: its observation apparatus is not live");
  }
}
