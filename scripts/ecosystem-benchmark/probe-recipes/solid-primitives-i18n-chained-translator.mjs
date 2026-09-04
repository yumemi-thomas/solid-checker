// Hand-authored probe recipe for the `creates: []` claim domain of
// `@solid-primitives/i18n@2.2.1`'s `chainedTranslator`, on the `.` artifact
// case whose runtime target is `dist/index.js`.
//
// A veto and nothing more: the implementation census proves `creates: []` or
// refuses it, and a clean run here is not evidence either way.
//
// `chainedTranslator` builds an object whose leaves are arrows that call the
// translator it was handed, and recurses into nested record dictionaries. So
// the dictionary is nested, and both a nested leaf and a top-level leaf are
// invoked: without calling a leaf, the arrows the export constructs never run,
// and the veto would have observed only the construction.
//
// The observable contradiction is the narrow one this corpus declares: the only
// runtime a `create` could register into from the worker's realm is the global
// object, since the worker has no DOM.
//
// It hands the package no `session` and no `harness`, and performs no `create`.

import { chainedTranslator } from "@solid-primitives/i18n";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });

  const before = Object.keys(globalThis).length;

  const paths = [];
  const translate = path => {
    paths.push(String(path));
    return `resolved:${String(path)}`;
  };

  const chained = chainedTranslator({ greetings: { hello: "hello" }, bye: "bye" }, translate);

  const nested = chained.greetings.hello();
  const top = chained.bye();

  if (nested !== "resolved:.greetings.hello") {
    throw new Error(`the nested leaf resolved ${JSON.stringify(String(nested))}`);
  }
  if (top !== "resolved:.bye") {
    throw new Error(`the top-level leaf resolved ${JSON.stringify(String(top))}`);
  }
  if (paths.length !== 2) {
    throw new Error(`the leaves called the translator ${paths.length} time(s)`);
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
