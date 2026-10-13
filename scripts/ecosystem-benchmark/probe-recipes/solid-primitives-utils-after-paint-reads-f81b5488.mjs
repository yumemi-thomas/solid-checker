// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`, on the
// published `.` runtime case `artifact-case:f81b5488e3996a8a5274855cc63234dc16f0708c5e95952296ff999003a4834a`
// (the case the ecosystem corpus certifies under solid-js@2.0.0-rc.3; the same
// dist/index.js bytes appear under two more `.` cases, each with its own copy of
// this module, because the private workspace writes one file per recipe entry).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `afterPaint` is `(fn) => { if (typeof requestAnimationFrame === "function")
// requestAnimationFrame(() => requestAnimationFrame(fn)); }`. It reads one
// global binding and, in a browser, hands the caller's `fn` to the host's
// frame scheduler twice removed. The pinned interpreter defines no
// `requestAnimationFrame`, so the samples assert the export returns
// `undefined` and that the caller's `fn` is never invoked, and pass a callback
// that would read an owned getter to show that. What the samples cannot see
// is the browser branch, where the read the scheduled `fn` performs is the
// caller's under ADR 0034 anyway.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { afterPaint } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  if (typeof requestAnimationFrame === "function") {
    throw new Error("the pinned interpreter unexpectedly defines requestAnimationFrame");
  }
  for (let sample = 0; sample < 3; sample += 1) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = afterPaint(() => ownedByTheRecipe.current);
    if (answered !== undefined) {
      throw new Error(`afterPaint answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  await Promise.resolve();
  if (observed !== 0) {
    throw new Error(`afterPaint ran the caller's callback ${observed} time(s) without a frame scheduler`);
  }
}
