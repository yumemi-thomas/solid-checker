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
