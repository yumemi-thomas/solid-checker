// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Demand-scoped: consumer projects import `trueFn` from this exact version
// (`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.md`).
//
// `trueFn` is `() => true`. It owns no source. The samples pass an owned
// getter object as an argument and assert it is neither read nor called.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { trueFn } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  for (const args of [[], [ownedByTheRecipe], [() => ownedByTheRecipe.current]]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = trueFn(...args);
    if (answered !== true) {
      throw new Error(`trueFn() answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (observed !== 0) {
    throw new Error(`trueFn read a caller-owned source ${observed} time(s)`);
  }
}
