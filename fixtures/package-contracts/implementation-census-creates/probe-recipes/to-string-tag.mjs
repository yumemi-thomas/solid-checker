// Hand-authored probe recipe for the `creates: []` claim domain of
// `toStringTagViaCall` (ADR 0034).
//
// `Object.prototype.toString.call(value)` reaches user code only through
// `value[Symbol.toStringTag]`; the census decides the `.call` by the reviewed
// this-protocol table and the parameter-rooted `this`. This recipe proves the
// export ran, on a string and on a plain object, and nothing more.

import { toStringTagViaCall } from "implementation-census-creates-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  toStringTagViaCall("text");
  toStringTagViaCall({});
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
