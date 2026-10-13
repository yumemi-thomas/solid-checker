// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Not named by any of the 118 consumer projects in the demand corpus; written
// because the second scaffold pass showed the census can decide this candidate
// and one recipe on this dependency node closes the entry in every row that
// depends on it (2026-09-13).
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
