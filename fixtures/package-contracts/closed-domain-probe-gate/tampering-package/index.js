// A package that attacks the transcript from inside the worker's realm.
//
// A recipe imports the package under test, so this module's top level runs in
// the same realm as the harness that reports what the probe observed — before
// a single event is recorded. Every name patched below was on the report path
// at some point in this design, and patching `structuredClone` alone was once
// enough to drop the contradiction event and renumber the rest into a
// transcript that looked clean.
//
// The report path no longer reaches any of them by name. Both the harness and
// the worker capture the primordials they need while their own modules
// evaluate, which is strictly before this file is imported, and the frames go
// to a descriptor this package cannot name — `process.stdout.write` reaches a
// stream nothing reads.
//
// So this package's runtime also *drifts*: `entry` is a number, which the
// declaration excludes. The recipe therefore has a real contradiction to
// report, and the test's claim is that the tampering cannot stop it being
// reported. A run that merely broke would prove much less.
// Captured before the replacement below, because the laundering arms at the
// end of this file need a stringify that actually works.
const realStringify = JSON.stringify;
const realParse = JSON.parse;

globalThis.structuredClone = () => [];

// A stringify that cannot serialize a frame. Node's own internals mostly avoid
// it, and if the worker did use it the run would fail rather than pass, which
// is still not a certification.
JSON.stringify = () => '{"outcome":{"kind":"completed","events":[]}}';

// The old frame channel. Writing here now reaches /dev/null under the
// certification harness, and a drained pipe under the audit driver.
process.stdout.write = () => true;

// The harness copies each event field by field with `Object.keys`, so a
// version that hides one key would hide the contradiction. Only `marker` is
// filtered, and only when present, so Node's own use of `Object.keys` keeps
// working and the attack is targeted rather than merely destructive.
const declaredKeys = Object.keys;
Object.keys = value => declaredKeys(value).filter(key => key !== "marker");

// The laundering arm, and the one that mattered most.
//
// `JSON.stringify` performs `Get(value, "toJSON")` on every object it visits
// and serializes whatever that returns. Capturing the function does not help:
// the lookup is part of the algorithm. So a `toJSON` installed on
// `Object.prototype` is handed the worker's *own run frame* — its real session
// id, environment and isolation — and can return a frame with the
// contradiction event removed and the rest renumbered. That produced a clean
// `CleanNonObservation` and the gate passed. `Array.prototype.toJSON` is the
// same attack aimed one level in, at the events container, which is the other
// object in the frame whose prototype was intact.
//
// Both are attempted inside a `try`, which a real attacker would not write.
// The worker freezes `Object.prototype`, `Array.prototype`, and
// `Function.prototype` before importing this module, so `defineProperty`
// throws; a package that let the throw escape would refuse the gate on a
// *failed run*, which proves much less than this fixture claims. Catching it
// keeps the claim the strong one — the contradiction is still observed and
// reported — and keeps the test sensitive to the frame representation itself
// rather than only to the freeze.
function suspendLaundering() {
  const object = Object.getOwnPropertyDescriptor(Object.prototype, "toJSON");
  const array = Object.getOwnPropertyDescriptor(Array.prototype, "toJSON");
  delete Object.prototype.toJSON;
  delete Array.prototype.toJSON;
  return () => {
    if (object) Object.defineProperty(Object.prototype, "toJSON", object);
    if (array) Object.defineProperty(Array.prototype, "toJSON", array);
  };
}

function withoutContradiction(events) {
  const kept = [];
  for (const event of events) {
    if (event && typeof event === "object") {
      if (event.marker === "undeclared-alternative") continue;
      if (typeof event.sequence === "number") event.sequence = kept.length;
    }
    kept.push(event);
  }
  return kept;
}

try {
  Object.defineProperty(Object.prototype, "toJSON", {
    configurable: true,
    writable: true,
    value() {
      const restore = suspendLaundering();
      try {
        // With both arms suspended this is an ordinary deep copy of whatever
        // object the worker is serializing, frame included.
        const copy = realParse(realStringify(this));
        if (copy && copy.outcome && Array.isArray(copy.outcome.events)) {
          copy.outcome.events = withoutContradiction(copy.outcome.events);
        }
        return copy;
      } finally {
        restore();
      }
    }
  });
  Object.defineProperty(Array.prototype, "toJSON", {
    configurable: true,
    writable: true,
    value() {
      const restore = suspendLaundering();
      try {
        return withoutContradiction(realParse(realStringify(this)));
      } finally {
        restore();
      }
    }
  });
} catch {
  // The worker froze the intrinsic prototypes before importing this module.
}

export const entry = 42;
