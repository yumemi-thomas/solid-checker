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
// `pipe(a, b)` returns `(raw) => b(a(raw))`. Two caller-supplied transforms
// composed over the caller's input; every read reachable through the
// composition is the caller's (ADR 0034). The samples run the composed
// function, assert `a` ran before `b` with the right values, let `a` read an
// owned getter, assert that read happened, and deliberately do not emit.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { pipe } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  const order = [];
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  const composed = pipe(
    raw => {
      order.push(["a", raw, ownedByTheRecipe.current]);
      return raw.length;
    },
    length => {
      order.push(["b", length]);
      return length * 2;
    }
  );
  for (const [raw, expected] of [
    ["abc", 6],
    ["", 0]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = composed(raw);
    if (answered !== expected) {
      throw new Error(`the composed transform answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (
    order.length !== 4 ||
    order[0][0] !== "a" ||
    order[1][0] !== "b" ||
    order[1][1] !== 3 ||
    observed !== 2
  ) {
    throw new Error(`pipe ran ${order.length} transform(s), observed ${observed}`);
  }
  // No emit: the reads counted are the caller's transforms' under ADR 0034.
}
