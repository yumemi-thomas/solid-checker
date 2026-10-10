// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`, on the
// published `.` runtime case `artifact-case:6cd714eb04fb05bfa0d7b88061837c8b505d6d91829178ccd8730a40be329b6d`
// (the case the ecosystem corpus certifies under solid-js@2.0.0-rc.3; the same
// dist/index.js bytes appear under two more `.` cases, each with its own copy of
// this module, because the private workspace writes one file per recipe entry).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `ofClass` is `(v, c) => v instanceof c || (v && v.constructor === c)`.
// The one member it reads is `v.constructor`, a property of the caller's
// value, reached only when `instanceof` was false. The samples cover a true
// instance, a foreign value, a nullish value (short-circuits before the read),
// and an owned object whose `constructor` getter this recipe counts: the read
// happens exactly once and is the caller's under ADR 0034, so it is not emitted.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { ofClass } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  class Marker {}
  let observed = 0;
  const ownedByTheRecipe = {
    get constructor() {
      observed += 1;
      return Marker;
    }
  };
  for (const [value, klass, expected] of [
    [new Marker(), Marker, true],
    [new Date(0), Marker, false],
    [null, Marker, false],
    [undefined, Marker, false],
    [ownedByTheRecipe, Marker, true]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = ofClass(value, klass);
    if (Boolean(answered) !== expected) {
      throw new Error(`ofClass(${String(value)}, Marker) answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (observed !== 1) {
    throw new Error(`the caller-owned constructor getter was read ${observed} time(s), expected 1`);
  }
  // No emit: that read is the caller's under ADR 0034.
}
