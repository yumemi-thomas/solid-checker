// Hand-authored probe recipe for the `creates: []` claim domain of
// `@solid-primitives/i18n@2.2.1`'s `scopedTranslator`, on the `.` artifact
// case whose runtime target is `dist/index.js`.
//
// The claim is *proved*, if at all, by the implementation census
// (`docs/adr/0008-implementation-census-for-creates.md`). This recipe cannot
// establish it and never tries to: a passing veto means only that nothing
// contradicted it, and a finite non-observation is never negative evidence.
//
// What the recipe does is exercise the whole implementation. `scopedTranslator`
// returns an arrow, so calling the export alone would run two statements and
// leave the arrow's own body — the only place a call happens — unexecuted. So
// the returned translator is called too, and its composed path asserted, which
// is what makes "nothing contradicted" a statement about the body rather than
// about the wrapper.
//
// The contradiction it can observe, stated exactly: a `create` under the
// version-1 model registers a resource into a runtime that outlives the call,
// and the only such runtime reachable from this realm is the global object
// itself — the worker has no DOM, so a `render`-style `register-delegation`
// could not succeed here even if the implementation attempted one. So the
// recipe counts `globalThis`'s own keys around the invocation and emits the
// `create-operation` marker when that set changes. That is a narrow tripwire
// and it is declared as one in `coverageLimitations`; it is not a substitute
// for the census.
//
// It hands the package no `session` and no `harness` (ADR 0006, "The worker's
// realm is the package's realm"), and performs no `create` of its own: the one
// value it passes in is a plain arrow over a module-local variable.

import { scopedTranslator } from "@solid-primitives/i18n";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });

  const before = Object.keys(globalThis).length;

  let observedPath = "";
  let observedArgument = "";
  const translate = (path, ...args) => {
    observedPath = String(path);
    observedArgument = String(args[0]);
    return `resolved:${String(path)}`;
  };

  const scoped = scopedTranslator(translate, "greetings");
  const answered = scoped("hello", "argument");

  if (observedPath !== "greetings.hello") {
    throw new Error(
      `scopedTranslator composed the path ${JSON.stringify(observedPath)}`
    );
  }
  if (observedArgument !== "argument") {
    throw new Error(
      `scopedTranslator forwarded the argument ${JSON.stringify(observedArgument)}`
    );
  }
  if (answered !== "resolved:greetings.hello") {
    throw new Error(
      `scopedTranslator returned ${JSON.stringify(String(answered))}`
    );
  }

  if (Object.keys(globalThis).length !== before) {
    harness.emit({
      marker: "create-operation",
      kind: "call",
      phase: "enter"
    });
  }

  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
