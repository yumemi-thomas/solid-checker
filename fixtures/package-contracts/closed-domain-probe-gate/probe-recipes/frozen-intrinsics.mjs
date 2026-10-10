// Hand-authored probe recipe that reports, from inside the worker's realm, the
// one realm property the harness design promises: `Object.prototype`,
// `Array.prototype`, and `Function.prototype` are frozen *before* a recipe —
// and therefore the package under test — is imported.
//
// # Why this exists
//
// The worker answers the `toJSON` laundering attack twice, independently: the
// whole frame is built as null-prototype records and serialized by the
// harness's own serializer, which consults no `toJSON` and no prototype; and
// the intrinsic prototypes are frozen, so a package that tries the patch throws
// instead of succeeding.
//
// Two independent answers are worth having and are hard to *test*, because
// each one hides the other. With the freeze in place a package's
// `Object.defineProperty(Object.prototype, "toJSON", …)` throws, so the
// laundering arm never runs and a test cannot tell a working serializer from a
// broken one; remove the freeze and the serializer ignores `toJSON` anyway, so
// the arm runs and changes nothing. There is no path that launders a frame
// while the freeze holds — a frame has no prototype chain for an inherited
// `toJSON` to sit on — so the two halves cannot both be exercised by one
// attack, and neither half was pinned by anything.
//
// So each is pinned directly and separately:
//
//   * the serializer, by `a frame is serialized without consulting toJSON or
//     any prototype` in `packages/cli/test/contract-workflow.test.mjs`, which
//     installs `Object.prototype.toJSON` in an ordinary unfrozen realm and
//     asserts that `JSON.stringify` launders while `serializeFrame` does not;
//   * the freeze, by this module and
//     `the_probe_gate_tracer_observes_frozen_intrinsics_in_the_workers_realm`.
//
// Removing either one now fails a test.
//
// # How it reports
//
// A recipe cannot assert; it can only emit events Rust classifies. The gate
// this module is registered for is a closure falsification for `entry`'s root
// choice alternatives, whose expected marker is `undeclared-alternative`. So an
// *unfrozen* prototype is emitted as that marker: the veto then fires and the
// row is refused, which is how the test observes the regression. When the
// design holds, nothing contradicts and the row certifies.
//
// The observation is taken at module scope, which is recipe-import time — the
// moment the worker hands control to package-reachable code — rather than
// inside `runProbeSession`, where a later thaw could no longer be attributed to
// the import boundary.

import { entry } from "closed-domain-probe-gate-package";

const objectPrototypeFrozen = Object.isFrozen(Object.prototype);
const arrayPrototypeFrozen = Object.isFrozen(Array.prototype);
const functionPrototypeFrozen = Object.isFrozen(Function.prototype);

export async function runProbeSession(_session, harness) {
  // The `call` pair is not decoration: Rust refuses an empty event transcript,
  // so a recipe has to prove it ran before its silence can mean anything. The
  // three booleans ride along so a failure says *which* prototype thawed.
  harness.emit({
    marker: "call",
    kind: "call",
    phase: "enter",
    objectPrototypeFrozen,
    arrayPrototypeFrozen,
    functionPrototypeFrozen
  });
  if (!objectPrototypeFrozen || !arrayPrototypeFrozen || !functionPrototypeFrozen) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  }
  // Imported and used, so this is a real run against the private snapshot copy
  // of the package rather than a bare interrogation of the realm. `entry` is a
  // callable, which is one of its two declared alternatives, so there is
  // nothing here to contradict.
  if (typeof entry !== "function" && entry !== undefined) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 1 });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
