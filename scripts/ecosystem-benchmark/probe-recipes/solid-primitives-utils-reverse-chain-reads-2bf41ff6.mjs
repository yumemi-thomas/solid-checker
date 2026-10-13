// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`, on the
// published `.` runtime case `artifact-case:2bf41ff69a22f6022ae4b8485df1bd52096d111e906fc32792e33004f7956ee0`
// (the case the ecosystem corpus certifies under solid-js@2.0.0-rc.0; the same
// dist/index.js bytes appear under two more `.` cases, each with its own copy of
// this module, because the private workspace writes one file per recipe entry).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `reverseChain(callbacks)` returns `(...args) => { for (let i =
// callbacks.length - 1; i >= 0; i--) { const callback = callbacks[i];
// callback && callback(...args); } }`. The returned function indexes the
// caller's array backwards and calls the caller's callbacks with the caller's
// arguments; every read reachable through it is the caller's (ADR 0034). The
// samples run the chained function, assert every callback ran in reverse
// order with the same arguments, pass a callback that reads an owned getter,
// assert that read happened, and deliberately do not emit.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { reverseChain } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  const order = [];
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  const chained = reverseChain([
    (a, b) => order.push(["first", a, b, ownedByTheRecipe.current]),
    undefined,
    (a, b) => order.push(["second", a, b])
  ]);
  for (let sample = 0; sample < 3; sample += 1) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = chained(sample, "b");
    if (answered !== undefined) {
      throw new Error(`the chained function answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (
    order.length !== 6 ||
    order.some((entry, index) => entry[0] !== (index % 2 === 0 ? "second" : "first")) ||
    observed !== 3
  ) {
    throw new Error(`reverseChain ran ${order.length} callback(s), observed ${observed}`);
  }
  // No emit: the reads counted are the caller's callbacks' under ADR 0034.
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  reverseChain([])();
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
