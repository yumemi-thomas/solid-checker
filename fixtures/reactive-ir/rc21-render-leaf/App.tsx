import { createEffect, createSignal, createStore, onSettled } from "solid-js";
import { render } from "@solidjs/web";


export function Settled() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  onSettled(() => {
    render(() => <span />, document.createElement("div"));
  });
  return <i>{store.v}</i>;
}

export function Apply() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    render(() => <span />, document.createElement("div"));
  });
  return <i>{store.v}</i>;
}

