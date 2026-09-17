// Hand-authored probe recipe for the `creates: []` claim domain of
// `spreadArgs` — the export whose spread the census now clears.
//
// The claim is *proved* by the implementation census: the one call
// `spreadArgs` reaches is the module-local `joinAll`, and the spread that
// passes its arguments drives `Array.prototype[Symbol.iterator]` on a rest
// parameter, whose type is `any[]` whatever its elements are. Both that
// factory and the array iterator it returns are engine code, so the producer
// records no uncensused form (`docs/typefacts/adr/0026-…`). This recipe cannot
// establish that and never tries to; a passing veto means only that nothing
// contradicted it.
//
// The `call` enter/exit pair is what makes the recipe's silence mean anything:
// Rust refuses an empty event transcript.

import { spreadArgs } from "implementation-census-creates-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  spreadArgs(1, 2);
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
