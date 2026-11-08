import { createEffect, createSignal, createStore } from "solid-js";
import { render } from "@solidjs/web";


export function Nested() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    setCount(5);
  });
  render(() => <span />, document.createElement("div"));
  return <i>{store.v}</i>;
}

