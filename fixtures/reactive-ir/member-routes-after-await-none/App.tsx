import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);

// The program holds no async computation, so no continuation runs a member: SC1002 has nothing to say about these reads.
const accessors = {
  get value() {
    return count();
  },
};
class Counter {
  get now() {
    return count();
  }
}
export function Sync() {
  const counter = new Counter();
  const m = createMemo(() => {
    void accessors.value;
    void counter.now;
    return 1;
  });
  return <div>{m()}</div>;
}

// Batch 10c (B2): the function-valued routes in a synchronous memo: no continuation, no SC1002 row.
const coercedArrow = { toString: () => String(count()) };
const assigned: { toString(): string } = {};
assigned.toString = () => String(count());
const trappedArrow = new Proxy({} as { value: number }, { get: () => count() });
export function SyncValues() {
  const m = createMemo(() => {
    void `${coercedArrow}`;
    void `${assigned}`;
    void trappedArrow.value;
    return 1;
  });
  return <div>{m()}</div>;
}
