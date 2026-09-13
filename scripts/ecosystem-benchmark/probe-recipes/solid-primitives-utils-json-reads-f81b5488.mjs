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
// `json` is `(raw) => JSON.parse(raw)`, with no reviver. It reads the
// caller's string and builds fresh values; nothing it touches is a source the
// package owns. The samples parse each JSON value kind and assert the shape,
// and one malformed input asserts the parser's own `SyntaxError` propagates.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { json } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  for (const [raw, check] of [
    ["1", answered => answered === 1],
    ['"a"', answered => answered === "a"],
    ["null", answered => answered === null],
    ["[1,2]", answered => Array.isArray(answered) && answered.length === 2 && answered[1] === 2],
    ['{"a":{"b":true}}', answered => answered.a.b === true]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = json(raw);
    if (!check(answered)) {
      throw new Error(`json(${raw}) answered ${JSON.stringify(answered)}`);
    }
    // No emit is the point: nothing contradicted the closure.
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let threw = false;
  try {
    json("{");
  } catch (error) {
    threw = error instanceof SyntaxError;
  }
  if (!threw) {
    throw new Error("json accepted a malformed document");
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
