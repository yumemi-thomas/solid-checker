// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`, on the
// published `.` runtime case `artifact-case:1bea9ecdd99dcdbedc17ea6efb689911ddcefe25cbd7a1c13424967dec04dab2`
// (the case the ecosystem corpus certifies under solid-js@2.0.0-rc.0; the same
// dist/index.js bytes appear under two more `.` cases, each with its own copy of
// this module, because the private workspace writes one file per recipe entry).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `safe(transform, fallback)` returns `(raw) => { try { return
// transform(raw); } catch { return fallback; } }`. The caller's transform over
// the caller's input, with the caller's fallback on a throw; every read
// reachable through it is the caller's (ADR 0034). The samples take both
// branches, let the transform read an owned getter, assert that read
// happened, and deliberately do not emit.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { safe } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  const guarded = safe(raw => {
    if (raw === "throw") {
      throw new Error("the transform threw");
    }
    return `${raw}:${ownedByTheRecipe.current}`;
  }, "fallback");
  for (const [raw, expected] of [
    ["ok", "ok:read"],
    ["throw", "fallback"],
    ["again", "again:read"]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = guarded(raw);
    if (answered !== expected) {
      throw new Error(`the guarded transform answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (observed !== 2) {
    throw new Error(`the caller-supplied transform read the owned getter ${observed} time(s)`);
  }
  // No emit: the reads counted are the caller's transform's under ADR 0034.
}
