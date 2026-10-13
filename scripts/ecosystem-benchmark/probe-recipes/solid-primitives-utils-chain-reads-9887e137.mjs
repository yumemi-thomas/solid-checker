// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Demand-scoped: consumer projects import `chain` from this exact version
// (`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.md`).
//
// `chain(callbacks)` returns `(...args) => { for (const callback of
// callbacks) callback && callback(...args); }`. The returned function
// iterates the caller's array and calls the caller's callbacks with the
// caller's arguments; every read reachable through it is the caller's
// (ADR 0034). The samples run the chained function, assert every callback
// ran in order with the same arguments, pass a callback that reads an owned
// getter, assert that read happened, and deliberately do not emit.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { chain } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  const order = [];
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  const chained = chain([
    (a, b) => order.push(["first", a, b]),
    undefined,
    (a, b) => order.push(["second", a, b, ownedByTheRecipe.current])
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
    order.some((entry, index) => entry[0] !== (index % 2 === 0 ? "first" : "second")) ||
    observed !== 3
  ) {
    throw new Error(`chain ran ${order.length} callback(s), observed ${observed}`);
  }
  // No emit: the reads counted are the caller's callbacks' under ADR 0034.
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  chain([])();
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
