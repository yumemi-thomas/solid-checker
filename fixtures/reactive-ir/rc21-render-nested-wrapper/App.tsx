import { createEffect, createSignal, createStore } from "solid-js";
import { render } from "@solidjs/web";


function mount(el: Element) {
  render(() => <span />, el);
}

export function Helper() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    setCount(5);
  });
  mount(document.createElement("div"));
  return <i>{store.v}</i>;
}

export function Rooted() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    setCount(5);
  });
  createRoot(() => {
    render(() => <span />, document.createElement("div"));
  });
  return <i>{store.v}</i>;
}

export function Memo() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    setCount(5);
  });
  const m = createMemo(() => {
    render(() => <span />, document.createElement("div"));
    return 1;
  });
  console.log(m);
  return <i>{store.v}</i>;
}

export function Handed() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    setCount(5);
  });
  [document.createElement("div")].forEach((el) => render(() => <span />, el));
  return <i>{store.v}</i>;
}

