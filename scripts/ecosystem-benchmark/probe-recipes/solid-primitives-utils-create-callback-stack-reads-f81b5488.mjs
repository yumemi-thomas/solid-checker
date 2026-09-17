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
// `createCallbackStack()` returns `{ push, execute, clear }` over a
// module-private array the *factory* owns per call. That array is a plain
// `Array`, not a reactive-shaped source: `push` appends, `execute` calls each
// pushed callback with the caller's four arguments and clears, `clear`
// replaces the array. The callbacks are the caller's, so every read reachable
// through `execute` is the caller's (ADR 0034). The samples assert the
// pushed callbacks ran once each with the right arguments and were cleared,
// pass a callback reading an owned getter, assert that read happened, and
// deliberately do not emit.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { createCallbackStack } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  const calls = [];
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const stack = createCallbackStack();
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
  if (typeof stack.push !== "function" || typeof stack.execute !== "function" || typeof stack.clear !== "function") {
    throw new Error("createCallbackStack did not answer a { push, execute, clear } object");
  }
  stack.push((a, b, c, d) => calls.push([a, b, c, d]));
  stack.push((a, b, c, d) => calls.push([a, b, c, d, ownedByTheRecipe.current]));
  stack.execute(1, 2, 3, 4);
  // A second execute after the clear runs nothing.
  stack.execute(5, 6, 7, 8);
  stack.push(() => calls.push(["late"]));
  stack.clear();
  stack.execute(9, 10, 11, 12);
  if (
    calls.length !== 2 ||
    calls[0].join() !== "1,2,3,4" ||
    calls[1].join() !== "1,2,3,4,read" ||
    observed !== 1
  ) {
    throw new Error(`createCallbackStack ran ${JSON.stringify(calls)}, observed ${observed}`);
  }
  // No emit: the read counted is the caller's callback's under ADR 0034.
}
