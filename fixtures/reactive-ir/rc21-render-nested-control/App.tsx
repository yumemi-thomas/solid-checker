import { createEffect, createSignal, createStore, onCleanup } from "solid-js";


export function Plain() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    onCleanup(() => {});
    setCount(5);
  });
  return <i>{store.v}</i>;
}

