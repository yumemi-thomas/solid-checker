// Hand-authored `reads: []` veto for `@floating-ui/utils@0.2.12`, on the published
// runtime case `artifact-case:9bc68a12911a31424edd543d041c3f76b21f2b2fdef6813016a8f0bf02379f43`
// (the ESM `.` case; the ecosystem corpus reaches it from three rows, and the
// package's second `.` case carries none of these candidates).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `createCoords` is `v => ({ x: v, y: v })`. It stores the caller's value twice
// and reads nothing from it; the samples pass an owned getter object and
// assert it was stored by identity and never read.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { createCoords } from "@floating-ui/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  for (const input of [0, 1.5, "a", ownedByTheRecipe, undefined]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = createCoords(input);
    if (!Object.is(answered.x, input) || !Object.is(answered.y, input)) {
      throw new Error(`createCoords(${String(input)}) answered ${JSON.stringify(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (observed !== 0) {
    throw new Error(`createCoords read a caller-owned source ${observed} time(s)`);
  }
}
