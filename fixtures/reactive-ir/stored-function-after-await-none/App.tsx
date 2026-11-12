import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);

// The program holds no async computation, so no continuation invokes what the program stored: SC1002 has nothing to say about
// these calls (a synchronous memo reads the same stored functions, tracked). SC1001's rows are as before.
const listeners: Array<() => unknown> = [];
listeners.push(() => count());
const bus = {
  listeners: [] as Array<() => unknown>,
  emit() {
    this.listeners.forEach((listener) => listener());
  },
};
bus.listeners.push(() => count());
export function Sync() {
  const m = createMemo(() => {
    listeners.forEach((listener) => listener());
    bus.emit();
    return 1;
  });
  return <div>{m()}</div>;
}

// Batch 10c: the other forms of invocation (a slot the program assigned, `.call`, a stored tag, a stored constructor) in a synchronous
// memo: the same, no continuation, no SC1002 row.
const slot: { cb: () => unknown } = { cb: () => {} };
slot.cb = () => count();
const held: Array<() => unknown> = [];
held.push(() => count());
const heldTags: Array<(strings: TemplateStringsArray) => unknown> = [];
heldTags.push(() => count());
const constructors: Array<new () => object> = [];
constructors.push((function () {
  count();
}) as unknown as new () => object);
export function SyncForms() {
  const m = createMemo(() => {
    slot.cb();
    held[0].call(undefined);
    void heldTags[0]`x`;
    const Stored = constructors[0];
    new Stored();
    return 1;
  });
  return <div>{m()}</div>;
}
