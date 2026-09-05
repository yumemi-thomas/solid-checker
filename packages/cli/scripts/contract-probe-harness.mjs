// Output-only worker harness. It assigns sequence numbers, performs the
// bounded drain instructions Rust issued, and owns the frame representation and
// serializer the worker reports with. Event meaning is deliberately left to the
// Rust evaluator.
//
// # Why every primordial is captured while this module evaluates
//
// The recipe imports the analyzed package into this realm, so package
// top-level code runs before any event is recorded and can replace anything
// the report path reaches by name. `Array.prototype.push`, `structuredClone`,
// `Promise.resolve`, and `setTimeout` were all reachable here, and patching
// `structuredClone` alone was enough to drop one recorded event and renumber
// the rest into a transcript that looked clean.
//
// Nothing on the report path may therefore be looked up by name after the
// recipe is imported. The bindings below are captured while this module
// evaluates, which is before the worker imports anything of the package's:
// the worker imports this module statically and the recipe dynamically, in
// that order.
//
// # Why the frame is serialized here rather than with `JSON.stringify`
//
// Capturing `JSON.stringify` is not enough, and that was a real hole rather
// than a theoretical one. `JSON.stringify` performs `Get(value, "toJSON")` on
// every object it visits and serializes whatever that returns, so a package
// top level doing
//
//     Object.defineProperty(Object.prototype, "toJSON", { value() { … } })
//
// receives `this === frame`, reads the real session, environment and isolation
// out of it, and returns a laundered copy with the contradiction event removed
// and the rest renumbered. The gate then passed. A `replacer` does not help:
// `toJSON` runs first. Nor does capturing the function, because the lookup is
// part of the algorithm, not of the binding.
//
// So a frame is *not* an ordinary object graph. `createFrameRecord` and
// `createFrameList` build null-prototype records — no prototype chain, so
// there is nowhere for an inherited `toJSON` to live — and `serializeFrame`
// walks them itself, by own key and by index, consulting no `toJSON` and no
// prototype. The captured `JSON.stringify` survives only as a scalar escaper:
// a string, a finite number and a boolean are primitives, not objects, so the
// algorithm performs no `toJSON` lookup on them at all, and hand-rolling the
// string escaping would be a worse bug risk than that fact is a hazard.
//
// The worker additionally freezes `Object.prototype`, `Array.prototype`, and
// `Function.prototype` before importing the recipe, so a package that tries
// the patch above throws instead. **Against the `toJSON` attack specifically,
// each of those two halves is sufficient on its own**, and that is the only
// claim being made: the freeze is not a general realm sandbox (a package can
// still replace any global, and in-realm loader hooks are not denied), and the
// frame representation is not a defence against anything but the report path.
//
// Neither half is load-bearing for the other, and neither is untested. The
// serializer is pinned by `a frame is serialized without consulting toJSON or
// any prototype` in `packages/cli/test/contract-workflow.test.mjs`, which
// installs `Object.prototype.toJSON` in a realm where nothing is frozen and
// asserts that `JSON.stringify` launders while `serializeFrame` does not. The
// freeze is pinned by the `frozen-intrinsics.mjs` recipe and
// `the_probe_gate_tracer_observes_frozen_intrinsics_in_the_workers_realm`,
// which reports `Object.isFrozen` from inside a launched worker at
// recipe-import time. Removing either one fails a test.
//
// Nothing on the report path *depends* on the freeze, though, and that is
// deliberate rather than incidental: the two places that once did — a `for…of`
// over `session.drain`, reaching `Array.prototype[Symbol.iterator]`, and a
// `typeof controls.flush` on a prototyped object, reaching `Object.prototype` —
// are an index loop and an own-property lookup below.
//
// # Why a recorded event is copied rather than cloned
//
// `emit` copies the object the recipe handed it, field by field, into a frozen
// null-prototype record of scalars. A getter, a proxy, or a later mutation
// therefore cannot change what is reported, and the copy needs no
// `structuredClone` — which is a global, and was the patched one. A non-scalar
// field is refused outright: every event the Rust decoder accepts is a flat
// record of strings, finite numbers, and booleans, so accepting more would
// only widen what a recipe can smuggle through.

const ownKeys = Object.keys;
const createRecord = Object.create;
const prototypeOf = Object.getPrototypeOf;
const freeze = Object.freeze;
// Own-property presence, so a lookup on a caller-supplied object never consults
// `Object.prototype`. The recipe's return value is such an object.
const hasOwnProperty = Object.hasOwn;
const isArray = Array.isArray;
const isFiniteNumber = Number.isFinite;
const TypeErrorConstructor = TypeError;
const PromiseConstructor = Promise;
const resolvePromise = Promise.resolve.bind(Promise);
const scheduleMacrotask = setTimeout;
// Calling a caller-supplied function without reaching `Function.prototype.call`
// by name. `Reflect` is a global like any other, so the binding is captured
// here rather than looked up after the recipe — and therefore the package — has
// run.
const applyFunction = Reflect.apply;
// Applied to primitives only. A primitive is neither an Object nor a BigInt,
// so the serialization algorithm never performs `Get(value, "toJSON")` on it
// and never reaches a prototype: this is a quoting function and nothing else.
const encodeScalar = JSON.stringify;

// A module-local symbol, so nothing outside this module can forge a frame
// list, and `Object.keys` never reports it.
const FRAME_LIST = Symbol("solid-checker-probe-frame-list");

// Shared by the worker and audit callers. Rust separately pins this protocol
// and the complete interpreted image before a certification launch.
export const PROBE_WORKER_PROTOCOL = "solid-checker-runtime-probe-v5";

/// An empty frame object: own string keys, no prototype.
export function createFrameRecord() {
  return createRecord(null);
}

/// An empty frame array. Deliberately not an `Array`: an array's prototype is
/// intact, which is one more place an inherited `toJSON` could sit, and the
/// events container was exactly such a place.
export function createFrameList() {
  const list = createRecord(null);
  list[FRAME_LIST] = true;
  list.length = 0;
  return list;
}

export function appendFrameItem(list, value) {
  list[list.length] = value;
  list.length += 1;
  return list;
}

function isFrameList(value) {
  return value[FRAME_LIST] === true;
}

/// Deep-copies plain JSON data — what `JSON.parse` produced for the session —
/// into frame records and lists.
///
/// Used for the transport fields the worker echoes back, which are read *before*
/// the recipe runs: the recipe is handed the parsed session, so a copy taken
/// afterwards would be a copy of whatever it left there.
///
/// The depth is bounded. The input is `JSON.parse` output, so it cannot be
/// cyclic, but it can be arbitrarily deep, and this walk is recursive: a
/// session document nested past the bound would otherwise overflow the stack
/// rather than refuse. The transport fields it is applied to — a session id and
/// the environment identity — are three levels deep at most.
const MAX_FRAME_DEPTH = 32;

export function adoptFrameValue(value, depth = 0) {
  if (value === null) return null;
  const kind = typeof value;
  if (kind === "string" || kind === "boolean") return value;
  if (kind === "number") {
    if (!isFiniteNumber(value)) {
      throw new TypeErrorConstructor("a probe frame number must be finite");
    }
    return value;
  }
  if (kind !== "object") {
    throw new TypeErrorConstructor(`a probe frame cannot carry a ${kind}`);
  }
  if (depth >= MAX_FRAME_DEPTH) {
    throw new TypeErrorConstructor(
      `a probe frame may not nest deeper than ${MAX_FRAME_DEPTH} levels`
    );
  }
  if (isArray(value)) {
    const list = createFrameList();
    for (let index = 0; index < value.length; index += 1) {
      appendFrameItem(list, adoptFrameValue(value[index], depth + 1));
    }
    return list;
  }
  const record = createFrameRecord();
  const keys = ownKeys(value);
  for (let index = 0; index < keys.length; index += 1) {
    record[keys[index]] = adoptFrameValue(value[keys[index]], depth + 1);
  }
  return record;
}

/// Serializes one frame to JSON text without consulting `toJSON` or any
/// prototype. A value that is not a scalar, a frame record, or a frame list is
/// refused rather than described, so an object that reached a frame by accident
/// fails the launch instead of being reported.
export function serializeFrame(value) {
  if (value === null) return "null";
  const kind = typeof value;
  if (kind === "string" || kind === "boolean") return encodeScalar(value);
  if (kind === "number") {
    if (!isFiniteNumber(value)) {
      throw new TypeErrorConstructor("a probe frame number must be finite");
    }
    return encodeScalar(value);
  }
  if (kind !== "object") {
    throw new TypeErrorConstructor(`a probe frame cannot carry a ${kind}`);
  }
  // The prototype check comes *first*, before the frame-list branch. Both
  // branches below read properties off `value`, and on a prototyped object that
  // is a lookup that can reach an inherited accessor or a proxy trap — so
  // `isFrameList` itself must not be the thing that decides whether an
  // ordinary object gets read. Every frame list is a null-prototype record, so
  // ordering the refusal ahead of the branch costs nothing.
  if (prototypeOf(value) !== null) {
    throw new TypeErrorConstructor("a probe frame may only carry null-prototype records");
  }
  if (isFrameList(value)) {
    let text = "[";
    for (let index = 0; index < value.length; index += 1) {
      if (index > 0) text += ",";
      text += serializeFrame(value[index]);
    }
    return `${text}]`;
  }
  let text = "{";
  const keys = ownKeys(value);
  for (let index = 0; index < keys.length; index += 1) {
    if (index > 0) text += ",";
    text += `${encodeScalar(keys[index])}:${serializeFrame(value[keys[index]])}`;
  }
  return `${text}}`;
}

function record(event, sequence) {
  if (!event || typeof event !== "object") {
    throw new TypeErrorConstructor("runtime probe events must be objects");
  }
  const copy = createFrameRecord();
  const keys = ownKeys(event);
  // By index: iterating the key array with `for…of` would reach
  // `Array.prototype[Symbol.iterator]`, which is a name again.
  for (let index = 0; index < keys.length; index += 1) {
    const key = keys[index];
    const value = event[key];
    const kind = typeof value;
    if (kind === "string" || kind === "boolean") {
      copy[key] = value;
      continue;
    }
    if (kind === "number" && isFiniteNumber(value)) {
      copy[key] = value;
      continue;
    }
    throw new TypeErrorConstructor(
      `runtime probe event field ${key} must be a string, a finite number, or a boolean`
    );
  }
  if (typeof copy.marker !== "string") {
    throw new TypeErrorConstructor("runtime probe events require a marker");
  }
  // Last, so a recipe cannot number its own events.
  copy.sequence = sequence;
  return freeze(copy);
}

export function createRuntimeProbeHarness(session) {
  const recorded = createFrameList();
  let microtasks = 0;
  let macrotasks = 0;
  return freeze({
    emit(event) {
      appendFrameItem(recorded, record(event, recorded.length));
    },
    async drain(controls) {
      // By index, and never `for…of`: iterating the planned drain steps with
      // `for…of` reads `Array.prototype[Symbol.iterator]`, which is a name the
      // package under test could have replaced. `session.drain` is
      // `JSON.parse` output, so it is a real array with an intact prototype —
      // exactly the shape that lookup would have reached.
      const steps = session.drain;
      if (!isArray(steps)) {
        throw new TypeErrorConstructor("a probe session's drain plan must be a list of steps");
      }
      for (let position = 0; position < steps.length; position += 1) {
        const step = steps[position];
        if (step === null || typeof step !== "object") {
          throw new TypeErrorConstructor("a probe session's drain step must be a record");
        }
        // Own properties again, for the same reason: a step is `JSON.parse`
        // output with an intact prototype, so a missing `kind` or `maxTurns`
        // would otherwise be answered by whatever a package put on
        // `Object.prototype`. Rust re-checks the drained counts against the
        // plan, so a manipulated step is a refusal rather than a pass — but the
        // lookup should not be reachable in the first place.
        const kind = hasOwnProperty(step, "kind") ? step.kind : undefined;
        const maxTurns = hasOwnProperty(step, "maxTurns") ? step.maxTurns : undefined;
        if (kind === "flush") {
          // An *own*-property lookup: the recipe's return value is an ordinary
          // object with an intact prototype, so `controls.flush` on a recipe
          // that returned no control would consult `Object.prototype` — and
          // find whatever a package installed there.
          const flush =
            controls !== null && controls !== undefined && hasOwnProperty(controls, "flush")
              ? controls.flush
              : undefined;
          if (typeof flush !== "function") {
            throw new TypeErrorConstructor(
              "runtime probe recipe did not provide the planned flush control"
            );
          }
          await applyFunction(flush, controls, []);
        } else if (kind === "microtasks") {
          for (let turn = 0; turn < maxTurns; turn += 1) {
            await resolvePromise();
            microtasks += 1;
          }
        } else if (kind === "macrotasks") {
          for (let turn = 0; turn < maxTurns; turn += 1) {
            await new PromiseConstructor(resolve => scheduleMacrotask(resolve, 0));
            macrotasks += 1;
          }
        } else {
          throw new TypeErrorConstructor(`unknown runtime probe drain step ${kind}`);
        }
      }
    },
    events() {
      // The records are frozen and null-prototype, so handing back a fresh
      // list is enough: nothing the recipe holds can change one afterwards.
      const copy = createFrameList();
      for (let index = 0; index < recorded.length; index += 1) {
        appendFrameItem(copy, recorded[index]);
      }
      return copy;
    },
    drainedMicrotasks: () => microtasks,
    drainedMacrotasks: () => macrotasks
  });
}
