// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Not named by any of the 118 consumer projects in the demand corpus; written
// because the second scaffold pass showed the census can decide this candidate
// and one recipe on this dependency node closes the entry in every row that
// depends on it (2026-09-13).
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
