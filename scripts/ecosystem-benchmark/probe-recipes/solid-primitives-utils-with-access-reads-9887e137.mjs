// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Not named by any of the 118 consumer projects in the demand corpus; written
// because the second scaffold pass showed the census can decide this candidate
// and one recipe on this dependency node closes the entry in every row that
// depends on it (2026-09-13).
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
