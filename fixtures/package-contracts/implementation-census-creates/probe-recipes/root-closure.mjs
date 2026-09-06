// Hand-authored probe recipe for the `creates: []` claim domain of the four
// ADR 0043 exports that certify: `defaultedFromParameter`, `patternParameter`,
// `localBindingFromParameter` and `localPatternFromParameter`.
//
// The claim is *proved* by the implementation census: each body's only
// invoking forms are reads of a value the caller passed, rooted through a
// default, a parameter pattern, or a local declaration. This recipe cannot
// establish that and never tries to; it proves the export ran, with plain data
// objects so that a passing veto is a clean one.
//
// One recipe serves all four because a gate names its claim, not its module:
// each export's own gate selects this file and calls exactly its own export.

import {
  defaultedFromParameter,
  localBindingFromParameter,
  localPatternFromParameter,
  patternParameter,
} from "implementation-census-creates-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  defaultedFromParameter({ min: 0 });
  defaultedFromParameter({ min: 0 }, { min: 1 });
  patternParameter({ inner: { value: 1 } });
  localBindingFromParameter({ inner: { value: { text: "t" } } });
  localPatternFromParameter({ inner: { value: 1 } });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
