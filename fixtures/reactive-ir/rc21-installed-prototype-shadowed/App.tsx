import { createSignal, createStore } from "solid-js";


export function Shadowed() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const Object = { create(p: unknown) { return { p }; } };
  const Reflect = { setPrototypeOf(o: unknown, p: unknown) { return [o, p]; } };
  console.log(Object.create(store), Reflect.setPrototypeOf({}, store));
  return <i>{store.v}</i>;
}

