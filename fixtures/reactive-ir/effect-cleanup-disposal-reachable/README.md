# effect-cleanup-disposal-reachable

**Claim (ADR 1167, the counterexample).** A `createRoot` that takes its `dispose` lets project code dispose an owner on a stack it
chooses (`DisposedInBody` disposes in the component body, inside the strict-read window): the never-rerunning effect's returned
cleanup is placed nowhere, SC1001 uncertifiable (ADR 0695). Chrome: `runtime-oracle/effect-cleanup-disposal-reachable`.
Stub: `effect-cleanup-disposal-only`'s.
