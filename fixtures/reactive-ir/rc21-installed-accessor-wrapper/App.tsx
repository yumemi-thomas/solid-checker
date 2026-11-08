import { createSignal, createStore } from "solid-js";


export function Alias() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  const dp = Object.defineProperty;
  dp(o, "a", { set: setCount });
  o.a = 2;
  return <i>{store.v}</i>;
}

export function Destructured() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  const { defineProperty } = Object;
  defineProperty(o, "a", { set: setCount });
  o.a = 2;
  return <i>{store.v}</i>;
}

export function Called() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  Object.defineProperty.call(Object, o, "a", { set: setCount });
  o.a = 2;
  return <i>{store.v}</i>;
}

export function LiteralKey() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  Object["defineProperty"](o, "a", { set: setCount });
  o.a = 2;
  return <i>{store.v}</i>;
}

export function Global() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  globalThis.Object.defineProperty(o, "a", { set: setCount });
  o.a = 2;
  return <i>{store.v}</i>;
}

function install(o: object, set: (v: number) => void) {
  Object.defineProperty(o, "a", { set });
}

export function Helper() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  install(o, setCount);
  o.a = 2;
  return <i>{store.v}</i>;
}

