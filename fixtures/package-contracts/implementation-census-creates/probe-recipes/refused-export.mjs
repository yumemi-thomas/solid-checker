// A recipe for an export whose `creates: []` claim the census REFUSES: `cycle`,
// `deep`, `unresolved`, `taggedTemplate`, `spreadArgs`, `switchBreak`,
// `whileBreak`, `stdlibRefInvoker`, `reflectApply`, and `reassignedHelper`.
//
// It exists to make the refusal tests honest rather than accidental. Without a
// recipe in the corpus the certifier withholds the candidate by name before
// any demand is planned, and the row certifies with the domain open — which is
// the right outcome for a missing recipe and the wrong test for a census
// refusal. With this recipe present the candidate is planned, the census runs,
// and the refusal it produces is the census's own. The gate is never reached,
// so what this module does when run does not matter to those tests; it is
// still a well-formed recipe.

import * as censused from "implementation-census-creates-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  censused.viaHelperChain(() => {});
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
