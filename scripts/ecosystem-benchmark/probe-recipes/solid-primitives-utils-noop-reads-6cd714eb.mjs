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
// `noop` is `() => void 0`. It owns no source and executes nothing the
// caller hands it. The samples pass arguments anyway and assert they are
// ignored, so a future version that started consulting one would be sampled
// by a recipe that can see a property read on a value it owns.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { noop } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  for (const args of [[], [1], [ownedByTheRecipe], [() => ownedByTheRecipe.current]]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = noop(...args);
    if (answered !== undefined) {
      throw new Error(`noop answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  // The apparatus is live (the getter is a real trap), and nothing read it:
  // `noop` neither accesses its arguments nor calls them.
  if (observed !== 0) {
    throw new Error(`noop read a caller-owned source ${observed} time(s)`);
  }
}
