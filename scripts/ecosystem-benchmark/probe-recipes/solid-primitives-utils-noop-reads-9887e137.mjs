// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Demand-scoped: consumer projects import `noop` from this exact version
// (`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.md`).
//
// `noop` is `() => void 0`. It owns no source and executes nothing the
// caller hands it. The samples pass arguments anyway and assert they are
// ignored, so a future version that started consulting one would be sampled
// by a recipe that can see a property read on a value it owns.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { noop } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  for (const args of [[], [1], [ownedByTheRecipe], [() => ownedByTheRecipe.current]]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = noop(...args);
    if (answered !== undefined) {
      throw new Error(`noop answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  // The apparatus is live (the getter is a real trap), and nothing read it:
  // `noop` neither accesses its arguments nor calls them.
  if (observed !== 0) {
    throw new Error(`noop read a caller-owned source ${observed} time(s)`);
  }
}
