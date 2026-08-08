import { createMemo, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

// Writing during a memo's computation corrupts the update graph: the memo is
// being evaluated, and the write schedules its dependents again. Solid 1.x
// forbids writes in tracked scopes for this reason. (A write inside
// `createEffect` is a different matter and is idiomatic 1.x.)
const doubled = createMemo(() => {
  setCount(count() + 1);
  return count() * 2;
});

export { doubled };
