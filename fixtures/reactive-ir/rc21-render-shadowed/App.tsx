import { createEffect, createSignal, createStore } from "solid-js";
import { render } from "@solidjs/web";


export function Shadowed() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    setCount(5);
  });
  const render = (fn: () => number) => fn();
  render(() => 1);
  return <i>{store.v}</i>;
}

