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
// `withAccess(value, fn)` is `const _value = access(value); typeof _value !=
// null && fn(_value);`. `typeof` never yields `null`, so `fn` runs on every
// call, `undefined` included -- the samples assert exactly that rather than
// the documented intent. A read reached through a caller-supplied accessor is
// the caller's (ADR 0034): the last sample passes an accessor that reads an
// owned getter, asserts it ran, and deliberately does not emit.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { withAccess } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  const received = [];
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  const zeroArity = () => "called";
  for (const [input, expected] of [
    [1, 1],
    [zeroArity, "called"],
    [undefined, undefined],
    [() => ownedByTheRecipe.current, "read"]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = withAccess(input, value => received.push(value));
    if (answered !== undefined) {
      throw new Error(`withAccess answered ${String(answered)}`);
    }
    if (!Object.is(received[received.length - 1], expected)) {
      throw new Error(`fn received ${String(received[received.length - 1])}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (received.length !== 4 || observed !== 1) {
    throw new Error(`fn ran ${received.length} time(s), observed ${observed}`);
  }
  // No emit: the read is the caller's accessor's under ADR 0034.
}
