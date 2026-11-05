import { createSignal, createStore } from "solid-js";


function make(s: object): any {
  return { __proto__: s };
}

export function Helper() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  console.log(make(store).v);
  return <i>{store.v}</i>;
}

export function MethodThis() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = { a: 1, install() { Object.setPrototypeOf(this, store); } };
  o.install();
  o.x = 1;
  return <i>{store.v}</i>;
}

export function MemberThis() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = { install() { (this as any).__proto__ = store; } };
  o.install();
  o.x = 1;
  return <i>{store.v}</i>;
}

export function HandedKey() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = { a: 1 };
  const put = (k: string, v: unknown) => { o[k] = v; };
  put("__proto__", store);
  o.x = 1;
  return <i>{store.v}</i>;
}

export function Hoisted() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = { a: 1 };
  reparent();
  o.x = 1;
  function reparent() {
    Object.setPrototypeOf(o, store);
  }
  return <i>{store.v}</i>;
}

