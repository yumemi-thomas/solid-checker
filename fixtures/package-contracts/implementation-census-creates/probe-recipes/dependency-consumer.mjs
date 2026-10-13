// Hand-authored probe recipe for the `creates: []` claim domain of
// `dependency-consumer/`'s `plainConsumer`.
//
// It is the one recipe in this repository whose *import* is the thing under
// test. The bare specifier below loads a package whose own module top level
// does `import { record } from "solid-js"`, so this module cannot even
// evaluate unless the private probe workspace carries the authenticated
// dependency closure beside the analyzed package's copy. Before that it was
// `ERR_MODULE_NOT_FOUND` — a failed run and a refused gate — which is why no
// consumer row could be probed at all.
//
// Two assertions, and both are refusal directions rather than evidence of
// closure. `plainConsumer` is called because the veto has to observe the export
// whose domain the census proved. `callsDependency` is called because
// resolving is not running: the value comes back through the dependency copy's
// own code, so a specifier that resolved *somewhere* while the bytes were not
// the authenticated ones throws here, the run fails, and the gate refuses.
//
// As always, a passing veto proves only that nothing contradicted the claim.
// The implementation census (`docs/adr/0008-implementation-census-for-creates.md`)
// is what proves `creates: []`: `plainConsumer`'s one call is parameter-rooted.

import {
  callsDependency,
  plainConsumer
} from "implementation-census-creates-dependency-consumer";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  plainConsumer(() => {});
  const answered = callsDependency("probe");
  if (answered !== "recorded:probe") {
    throw new Error(
      `the authenticated dependency copy did not answer: ${String(answered)}`
    );
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
