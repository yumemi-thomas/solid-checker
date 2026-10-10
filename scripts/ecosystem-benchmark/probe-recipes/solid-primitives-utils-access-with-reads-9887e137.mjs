// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Demand-scoped: consumer projects import `accessWith` from this exact version
// (`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.md`).
//
// `accessWith(valueOrFn, ...args)` is `typeof valueOrFn === "function" ?
// valueOrFn(...args) : valueOrFn`. Like `access` it executes caller code and
// forwards the caller's arguments; every read that can happen through it is
// the caller's (ADR 0034). The last sample proves the apparatus live and
// deliberately does not emit.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { accessWith } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  const plain = { value: 1 };
  for (const [args, expected] of [
    [[plain], plain],
    [[undefined], undefined],
    [[0, 1, 2], 0],
    [[(a, b) => a + b, 2, 3], 5],
    [[() => "called"], "called"]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = accessWith(...args);
    if (!Object.is(answered, expected)) {
      throw new Error(`accessWith answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }

  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const answered = accessWith(source => source.current, ownedByTheRecipe);
  if (answered !== "read" || observed !== 1) {
    throw new Error(
      `the caller-supplied function did not run: answered ${String(answered)}, observed ${observed}`
    );
  }
  // No emit: the read is the caller's under ADR 0034.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
