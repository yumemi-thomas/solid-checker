// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Demand-scoped: consumer projects import `access` from this exact version
// (`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.md`).
//
// `access` is `typeof v === "function" && !v.length ? v() : v`. It is the
// export in this set that executes caller code, so it is where the domain
// boundary is: a read reached through the caller-supplied accessor is the
// caller's (ADR 0034), not this closure's. The last sample passes an accessor
// that reads a getter this recipe owns, asserts the getter ran, and
// deliberately does not emit -- which is what proves the apparatus live
// without contradicting a closure about sources the *package* owns.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { access } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  const zeroArity = () => "called";
  const oneArity = _unused => "not called";
  const plain = { value: 1 };

  for (const [input, expected] of [
    [plain, plain],
    [zeroArity, "called"],
    [oneArity, oneArity],
    [undefined, undefined],
    [0, 0]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = access(input);
    if (!Object.is(answered, expected)) {
      throw new Error(`access answered ${String(answered)}`);
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
  const answered = access(() => ownedByTheRecipe.current);
  if (answered !== "read" || observed !== 1) {
    throw new Error(
      `the caller-supplied accessor did not run: answered ${String(answered)}, observed ${observed}`
    );
  }
  // No emit: the read is the caller's under ADR 0034.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
