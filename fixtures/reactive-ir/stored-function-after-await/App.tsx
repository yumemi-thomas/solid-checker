import { createEffect, createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
const pause = () => Promise.resolve();

// ADR 0964 (batch 10b): the continuation of an async computation may invoke a function the program stored where nothing follows
// it (an array, a map, an object's list, a variable) through the name it was stored under. That call resolves to no function
// the walk knows, so a source read in the stored function warns UNTRACKED_READ_AFTER_AWAIT (Chrome, rc.21) while the program
// was certified clean beside SC1001's uncertifiable row at the read. The unresolved invocation after the await is an
// uncertifiable row of SC1002 (`unresolved-invocation`), never a violation (nothing proves what was stored) and never silence.

// Each shape stores its own function: the runtime warns once for a read site, so a function shared by two shapes is judged for
// the first only.

// The array iterated by a method.
const iterated: Array<() => unknown> = [];
iterated.push(() => count());
export function Foreach() {
  const m = createMemo(async () => {
    await pause();
    iterated.forEach((listener) => listener());
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// An element called by index (a computed call target: SC9012's own row, not this rule's).
const indexed: Array<() => unknown> = [];
indexed.push(() => count());
export function Indexed() {
  const m = createMemo(async () => {
    await pause();
    indexed[0]();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// An element held in a variable.
const aliased: Array<() => unknown> = [];
aliased.push(() => count());
export function Aliased() {
  const m = createMemo(async () => {
    await pause();
    const first = aliased[0];
    first();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// A map's value, called on the spot.
const registry = new Map<string, () => unknown>();
registry.set("count", () => count());
export function Mapped() {
  const m = createMemo(async () => {
    await pause();
    registry.get("count")!();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Tasks a promise combinator starts: the invocation is in the callback `map` runs.
const tasks: Array<() => Promise<unknown>> = [];
tasks.push(async () => count());
export function Awaiting() {
  const m = createMemo(async () => {
    await pause();
    await Promise.all(tasks.map((task) => task()));
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// An object's method that iterates its own list: the invocation is inside the method, which the continuation runs by calling it.
const bus = {
  listeners: [] as Array<() => unknown>,
  emit() {
    this.listeners.forEach((listener) => listener());
  },
};
bus.listeners.push(() => count());
export function Emitting() {
  const m = createMemo(async () => {
    await pause();
    bus.emit();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// A class's method that does the same.
class Bus {
  listeners: Array<() => unknown> = [];
  emit() {
    this.listeners.forEach((listener) => listener());
  }
}
const classBus = new Bus();
classBus.listeners.push(() => count());
export function ClassEmitting() {
  const m = createMemo(async () => {
    await pause();
    classBus.emit();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// A parameter the caller supplies: a violation where the walk follows the argument (ADR 0669), as before this ADR.
function run(callback: () => unknown) {
  callback();
}
export function Parameter() {
  const m = createMemo(async () => {
    await pause();
    run(() => count());
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Batch 10c (B1): the same fact written another way. Each form below invokes a function the program stored, and was silent for SC1002
// (Chrome rc.21 warns at the read in each). Each stores its own function.

// A slot the program assigned: a member of a typed object, an optional one, and the slot run through `.call`.
const slot: { cb: () => unknown } = { cb: () => {} };
slot.cb = () => count();
export function Slot() {
  const m = createMemo(async () => {
    await pause();
    slot.cb();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const optionalSlot: { cb?: () => unknown } = {};
optionalSlot.cb = () => count();
export function OptionalSlot() {
  const m = createMemo(async () => {
    await pause();
    optionalSlot.cb?.();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const slotCalled: { cb: () => unknown } = { cb: () => {} };
slotCalled.cb = () => count();
export function SlotCall() {
  const m = createMemo(async () => {
    await pause();
    slotCalled.cb.call(undefined);
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// An interface's method signature has no body: what runs is whatever object the program made to fit it (here a function the object
// names; a closed literal with the arrow written in place is a function the walk follows, a violation).
interface Service {
  run(): unknown;
}
function runCount() {
  return count();
}
const service: Service = { run: runCount };
export function InterfaceSlot() {
  const m = createMemo(async () => {
    await pause();
    service.run();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// `.call` and `.apply` run the value they are read from: an element held in a variable, an element read in place.
const dispatched: Array<() => unknown> = [];
dispatched.push(() => count());
export function Dispatch() {
  const m = createMemo(async () => {
    await pause();
    const callback = dispatched[0];
    callback.call(undefined);
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const applied: Array<() => unknown> = [];
applied.push(() => count());
export function Apply() {
  const m = createMemo(async () => {
    await pause();
    applied[0].apply(undefined, []);
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// A `let` the program assigns again holds the later function, not its initializer.
function readCount() {
  return count();
}
let reassigned: () => unknown = () => {};
reassigned = readCount;
export function Reassigned() {
  const m = createMemo(async () => {
    await pause();
    reassigned();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// A tag the program stored (a tagged template calls its tag where it is written): in a variable, and an element read in place.
const heldTags: Array<(strings: TemplateStringsArray) => unknown> = [];
heldTags.push(() => count());
export function StoredTag() {
  const m = createMemo(async () => {
    await pause();
    const held = heldTags[0];
    void held`x`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const indexedTags: Array<(strings: TemplateStringsArray) => unknown> = [];
indexedTags.push(() => count());
export function StoredTagIndexed() {
  const m = createMemo(async () => {
    await pause();
    void indexedTags[0]`x`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// A function the program stored as a constructor: `new` runs it.
const constructors: Array<new () => object> = [];
constructors.push((function () {
  count();
}) as unknown as new () => object);
export function StoredNew() {
  const m = createMemo(async () => {
    await pause();
    const Stored = constructors[0];
    new Stored();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Silent. Nothing was stored: a `let` nothing writes again holds its initializer, a class and a tag function the program declared are
// read where they are, `Math.max.apply` is the standard function, and `ownedMethods.run` is a method with a body the walk follows.
let steady = () => 3;
class Local {}
function localTag(_strings: TemplateStringsArray) {
  return 4;
}
const ownedMethods = {
  run() {
    return 5;
  },
};
export function Declared() {
  const m = createMemo(async () => {
    await pause();
    steady();
    new Local();
    void localTag`x`;
    Math.max.apply(null, [1, 2]);
    ownedMethods.run();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Silent. Invoked before the await: the read is a dependency of the computation, tracked.
const early: Array<() => unknown> = [];
early.push(() => count());
export function Before() {
  const m = createMemo(async () => {
    early.forEach((listener) => listener());
    await pause();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Silent. A handler an event runs, never the continuation.
const handled: Array<() => unknown> = [];
handled.push(() => count());
export function Handler() {
  const m = createMemo(async () => {
    await pause();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <button onClick={() => handled.forEach((listener) => listener())}>go</button>;
}

// Silent. The callee is a function the program wrote: a function declaration and a `const` bound to an arrow; each is read where
// it is, none is a value somebody stored.
function localHelper() {
  return 1;
}
const localArrow = () => 2;
export function Resolved() {
  const m = createMemo(async () => {
    await pause();
    localHelper();
    localArrow();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
