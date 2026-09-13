// Hand-authored `reads: []` veto for `@floating-ui/utils@0.2.12`, on the published
// runtime case `artifact-case:9bc68a12911a31424edd543d041c3f76b21f2b2fdef6813016a8f0bf02379f43`
// (the ESM `.` case; the ecosystem corpus reaches it from three rows, and the
// package's second `.` case carries none of these candidates).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `evaluate(value, param)` is `typeof value === "function" ? value(param) :
// value`. The only code it can run is the caller's `value`; the last sample
// passes a function that reads an owned getter through `param`, asserts the
// read happened, and deliberately does not emit: it is the caller's under
// ADR 0034.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { evaluate } from "@floating-ui/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  const plain = { value: 1 };
  for (const [value, param, expected] of [
    [plain, undefined, plain],
    [7, ownedByTheRecipe, 7],
    [undefined, 1, undefined],
    [p => p * 2, 4, 8]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = evaluate(value, param);
    if (!Object.is(answered, expected)) {
      throw new Error(`evaluate answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (observed !== 0) {
    throw new Error(`evaluate read a caller-owned source without being handed a function`);
  }
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const answered = evaluate(p => p.current, ownedByTheRecipe);
  if (answered !== "read" || observed !== 1) {
    throw new Error(`the caller-supplied function did not run: ${String(answered)}, observed ${observed}`);
  }
  // No emit: the read is the caller's under ADR 0034.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
