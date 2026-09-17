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
