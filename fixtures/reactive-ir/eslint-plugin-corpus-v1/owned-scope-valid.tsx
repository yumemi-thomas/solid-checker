import { createEffect, createMemo, createRoot, createSignal, untrack } from "solid-js";

const [count, setCount] = createSignal(0);

createRoot(() => {
  // Solid 1.x: writing inside an effect callback is ordinary practice.
  createEffect(() => setCount(count() + 1));
});

// An explicit untrack around the write states the intent.
createMemo(() => untrack(() => setCount(2)));

// A local function that merely looks like createSignal is not Solid's.
function localCreateSignal<T>(value: T) {
  return [() => value, (_next: T) => {}] as const;
}
const [local, setLocal] = localCreateSignal(0);
createMemo(() => setLocal(local() + 1));
