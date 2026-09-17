// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Demand-scoped: consumer projects import `isObject` from this exact version
// (`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.md`).
//
// `isObject` is `value !== null && (typeof value === "object" || typeof
// value === "function")`. A `typeof` reads no property. The apparatus passes an
// object with an owned getter and asserts it never ran.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { isObject } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  for (const [input, expected] of [
    [ownedByTheRecipe, true],
    [() => 1, true],
    [null, false],
    [undefined, false],
    [1, false],
    ["object", false]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = isObject(input);
    if (answered !== expected) {
      throw new Error(`isObject(${String(input)}) answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (observed !== 0) {
    throw new Error(`isObject read a caller-owned source ${observed} time(s)`);
  }
}
