// Hand-authored probe recipe for the `creates: []` claim domain of
// `@solid-primitives/i18n@2.2.1`'s `flatten`, on the `.` artifact case whose
// runtime target is `dist/index.js`.
//
// As with every closure-falsification recipe, this one can only veto: the
// implementation census is what proves `creates: []`, and a run that observes
// no contradiction proves nothing at all.
//
// `flatten` walks a nested dictionary, so the dictionary handed in is nested
// two levels deep: that is what makes the module-local `visitDict` recursion
// run rather than only the export's own loop. The flattened keys are asserted
// so a run that silently did nothing fails instead of passing the veto.
//
// The observable contradiction is the same narrow one every recipe in this
// corpus has, and it is declared in `coverageLimitations`: a version-1 `create`
// registers a resource into a runtime that outlives the call, and the only such
// runtime reachable from the worker's realm is the global object.
//
// It hands the package no `session` and no `harness`, and performs no `create`.

import { flatten } from "@solid-primitives/i18n";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });

  const before = Object.keys(globalThis).length;

  const flat = flatten({ a: { foo: "foo", b: { bar: 1 } } });

  if (flat["a.foo"] !== "foo") {
    throw new Error(`flatten did not flatten a.foo: ${JSON.stringify(flat)}`);
  }
  if (flat["a.b.bar"] !== 1) {
    throw new Error(`flatten did not recurse to a.b.bar: ${JSON.stringify(flat)}`);
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
