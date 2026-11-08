import { createEffect, createSignal, createStore, onCleanup } from "solid-js";
import { render } from "@solidjs/web";


export function Nested() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    onCleanup(() => {});
  });
  render(() => <span />, document.createElement("div"));
  return <i>{store.v}</i>;
}

