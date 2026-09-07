// Hand-authored probe recipe for the `creates: []` claim domain of the exports
// ADR 0043 and ADR 0044 certify: `defaultedFromParameter`, `patternParameter`,
// `localBindingFromParameter`, `localPatternFromParameter`, `ownTableRead`,
// `ownArrayRead`, `ownTableWrite`, `ownRestSpread`, `patternRestParameter` and
// the three ADR 0045 coercion exports and the three ADR 0047 instanceof ones.
//
// The claim is *proved* by the implementation census: each body's only
// invoking forms are reads of a value the caller passed, rooted through a
// default, a parameter pattern, or a local declaration. This recipe cannot
// establish that and never tries to; it proves the export ran, with plain data
// objects so that a passing veto is a clean one.
//
// One recipe serves them all because a gate names its claim, not its module:
// each export's own gate selects this file and calls exactly its own export.

import {
  coerceBoundHelperResult,
  readBoundCallerResult,
  readCallerResult,
  instanceOfLibrary,
  instanceOfOwnClass,
  instanceOfParameter,
  coerceConditionalHelperResult,
  coerceHelperResult,
  defaultedFromParameter,
  localBindingFromParameter,
  localPatternFromParameter,
  ownArrayRead,
  ownRestSpread,
  ownTableRead,
  ownTableWrite,
  patternParameter,
  patternRestParameter,
} from "implementation-census-creates-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  defaultedFromParameter({ min: 0 });
  defaultedFromParameter({ min: 0 }, { min: 1 });
  patternParameter({ inner: { value: 1 } });
  localBindingFromParameter({ inner: { value: { text: "t" } } });
  localPatternFromParameter({ inner: { value: 1 } });
  // ADR 0044, the same way: the claim is the census's, and these prove the
  // exports ran over a value this module built.
  ownTableRead("first");
  ownArrayRead(0);
  ownTableWrite("third");
  ownRestSpread({ first: undefined, other: 1 });
  patternRestParameter({ first: undefined, value: 1 });
  // ADR 0045, the same way: the claim is the census's; these prove the exports
  // ran over a helper whose completion is a number.
  coerceHelperResult(2);
  coerceBoundHelperResult(2);
  coerceConditionalHelperResult(2, 3);
  // ADR 0047.
  instanceOfParameter(1, Number);
  instanceOfLibrary(new Error("x"));
  instanceOfOwnClass(1);
  // ADR 0048.
  readCallerResult((p) => ({ y: p }), 1);
  readBoundCallerResult((p) => ({ y: p }), 1);
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
