import { createSignal, createStore } from "solid-js";


export function Define() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = { a: 1 };
  Object.defineProperty(o, "a", { get: count, set: setCount });
  o.a = 2;
  return <i>{store.v}</i>;
}

export function Many() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  Object.defineProperties(o, { a: { get: count, set: setCount } });
  o.a = 2;
  return <i>{store.v}</i>;
}

export function Reflected() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  Reflect.defineProperty(o, "a", { get: count, set: setCount });
  o.a = 2;
  return <i>{store.v}</i>;
}

export function Create() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o = Object.create({}, { a: { get: count, set: setCount } });
  o.a = 2;
  return <i>{store.v}</i>;
}

export function Held() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  const d = { get: count, set: setCount };
  Object.defineProperty(o, "a", d);
  o.a = 2;
  return <i>{store.v}</i>;
}

export function Compound() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  Object.defineProperty(o, "a", { get: () => 1, set: setCount });
  o.a += 2;
  o.a++;
  return <i>{store.v}</i>;
}

export function Legacy() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  const o: any = {};
  o.__defineSetter__("a", setCount);
  o.a = 2;
  return <i>{store.v}</i>;
}

