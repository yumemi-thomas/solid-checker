// A recipe for an export whose `returns: []` claim the census REFUSES:
// `returnsValue`, `expressionArrow`, `asyncVoid`, `generatorVoid`, and
// `valueReturnInLoop`.
//
// It exists to make the refusal tests honest rather than accidental. Without a
// recipe in the corpus the certifier withholds the candidate by name before
// any demand is planned, and the row certifies with the domain open — the
// right outcome for a missing recipe and the wrong test for a census refusal.
// With this recipe present the candidate is planned, the census runs, and the
// refusal it produces is the census's own. The gate is never reached.

import * as censused from "implementation-census-returns-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  censused.bareCompletion(1);
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
