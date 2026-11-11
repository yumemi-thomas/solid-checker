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
