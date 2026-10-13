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
// `createIdGenerator` is `() => { let seq = 0; const rand =
// Math.random().toString(36).slice(2, 8); return () => \`${Date.now()
// .toString(36)}-${(++seq).toString(36)}-${rand}\`; }`. It reads the host
// clock and the host's random source — neither is a reactive source, and
// neither is anything this recipe owns. The samples assert the shape of the
// ids, that the sequence segment advances, and that two generators do not
// share a sequence.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { createIdGenerator } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  const shape = /^[0-9a-z]+-([0-9a-z]+)-[0-9a-z]{1,6}$/;
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const generate = createIdGenerator();
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
  if (typeof generate !== "function") {
    throw new Error("createIdGenerator did not answer a function");
  }
  const first = generate();
  const second = generate();
  const other = createIdGenerator()();
  for (const id of [first, second, other]) {
    if (!shape.test(id)) {
      throw new Error(`generated id ${JSON.stringify(id)} has an unexpected shape`);
    }
  }
  if (first === second || shape.exec(first)[1] !== "1" || shape.exec(second)[1] !== "2") {
    throw new Error(`the sequence segment did not advance: ${first} then ${second}`);
  }
  if (shape.exec(other)[1] !== "1") {
    throw new Error(`a second generator shared the first one's sequence: ${other}`);
  }
  // No emit is the point: nothing contradicted the closure.
}
