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
