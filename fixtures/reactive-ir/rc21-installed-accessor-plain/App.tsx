import { createSignal, createStore } from "solid-js";


export function Plain() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  let held = 0;
  const o: any = { a: 1 };
  Object.defineProperty(o, "a", { value: 2, writable: true });
  Object.defineProperty(o, "b", { get: () => 1, set(v: number) { held = v; } });
  o.b = 3;
  console.log(o.a, held);
  return <i>{store.v}</i>;
}

export function Shadowed() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const Object = { defineProperty(o: unknown, k: string, d: unknown) { return [o, k, d]; } };
  Object.defineProperty({}, "a", { get: count, set: setCount });
  return <i>{store.v}</i>;
}

