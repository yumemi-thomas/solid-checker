// Hand-authored probe recipe for the `creates: []` claim domain of `plain`.
//
// The claim is *proved* by the implementation census: every call `plain`
// reaches — its own `callback(0)`, the local `mapAll`, and inside it
// `Array.from` and `values.map` — is dispositioned by its callee, and none of
// them can perform a `create` operation. This recipe cannot establish that and
// never tries to; a passing veto means only that nothing contradicted it.
//
// What it does is prove it ran: Rust refuses an empty event transcript, so the
// `call` enter/exit pair is what lets the recipe's silence about a
// contradiction mean anything at all.

import { plain } from "implementation-census-creates-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  plain([1, 2], value => value * 2);
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
