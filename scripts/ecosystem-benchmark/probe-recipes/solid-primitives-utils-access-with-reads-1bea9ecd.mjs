// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`, on the
// published `.` runtime case `artifact-case:1bea9ecdd99dcdbedc17ea6efb689911ddcefe25cbd7a1c13424967dec04dab2`
// (the case the ecosystem corpus certifies under solid-js@2.0.0-rc.0; the same
// dist/index.js bytes appear under two more `.` cases, each with its own copy of
// this module, because the private workspace writes one file per recipe entry).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
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
