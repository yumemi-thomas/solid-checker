import { createSignal, createStore } from "solid-js";


export function Plain() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const a = { __proto__: null, v: 1 };
  const b: any = { __proto__: { v: 1 } };
  const c = Object.create(null);
  const d = Object.create(Array.prototype);
  const e: any = {};
  Object.setPrototypeOf(e, { a: 1 });
  console.log(a.v, b.v, c, d, e);
  return <i>{store.v}</i>;
}

export function OwnProperty() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const __proto__ = store;
  console.log(({ ["__proto__"]: store } as any).v, ({ __proto__ } as any).v);
  return <i>{store.v}</i>;
}

