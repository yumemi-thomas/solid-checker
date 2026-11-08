import { createSignal, createStore, merge } from "solid-js";


export function Literal() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  console.log(({ __proto__: store } as any).v);
  return <i>{store.v}</i>;
}

export function Quoted() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  console.log(({ "__proto__": store } as any).v);
  return <i>{store.v}</i>;
}

export function Merged() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  console.log((merge({ __proto__: store } as any, { w: 2 }) as any).v);
  return <i>{store.v}</i>;
}

export function Created() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  console.log(Object.create(store).v);
  Object.create(store).v = 2;
  return <i>{store.v}</i>;
}

export function SetPrototype() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = { a: 1 };
  Object.setPrototypeOf(o, store);
  o.x = 1;
  return <i>{store.v}</i>;
}

export function ReflectSetPrototype() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = { a: 1 };
  Reflect.setPrototypeOf(o, store);
  console.log(o.v);
  return <i>{store.v}</i>;
}

export function Member() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = { a: 1 };
  o.__proto__ = store;
  o.x = 1;
  return <i>{store.v}</i>;
}

export function Receiver() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  Reflect.set({}, "v", 2, store);
  return <i>{store.v}</i>;
}

