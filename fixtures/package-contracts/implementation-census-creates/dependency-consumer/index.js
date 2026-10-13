// A consuming package that *imports a dependency at module top level*, which
// is the shape every real consumer row has and the one no probe could reach
// before the private workspace carried the authenticated dependency closure.
//
// The point of this file is what happens when a recipe writes
// `import … from "implementation-census-creates-dependency-consumer"` inside
// the private probe directory: the module below evaluates, its own
// `import "solid-js"` runs immediately, and unless that specifier resolves
// inside the authenticated private copy the module never loads at all —
// `ERR_MODULE_NOT_FOUND`, a failed run, a refused gate. That was the state
// `docs/adr/0008-implementation-census-for-creates.md` § "What this does not
// yet buy on real rows" recorded.
//
// It lives in its own package rather than beside `plain` and its siblings
// because those exports' recipes must keep proving the *no-dependency* path:
// a top-level `import "solid-js"` in the census fixture's own `index.js` would
// make every one of them depend on the closure this fixture is here to isolate.
import { record } from "solid-js";

// The census subject. Its one call is parameter-rooted, so the implementation
// census proves `creates: []` (ADR 0008 § 2) and the mandatory veto below
// actually runs — which is the only way the workspace mechanism gets exercised
// end to end. Deliberately *not* a call into `record`: a callee resolving into
// a dependency archive is refused by name ("a dependency export without an
// audited negative row"), the gate would never be reached, and this fixture
// would prove nothing about the workspace.
export function plainConsumer(callback) {
  callback(0);
}

// Not the census subject, and never demanded closed. The recipe calls it to
// prove the dependency copy's own code *ran* — that the specifier resolved to
// authenticated bytes rather than merely resolving somewhere — and refuses the
// gate when the answer is not the stub's.
export function callsDependency(value) {
  return record(value);
}
