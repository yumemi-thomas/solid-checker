import { createSignal } from "solid-js";

declare function ext(): number;

// Element call: the carrier's element is called in the body.
export function ElementCall() {
  const [a] = createSignal(1);
  const o = { a };
  const v = Object.values(o)[0]();
  console.log(v);
  return <p>{a()}</p>;
}

// Twin: the element is read but never called; only the carrier rows.
export function ElementHeld() {
  const [a] = createSignal(1);
  const o = { a };
  const kind = typeof Object.values(o)[0];
  console.log(kind);
  return <p>{a()}</p>;
}

// `for…of` over a standard call on a carrier: the loop binding is called.
export function ForOf() {
  const [a] = createSignal(1);
  const accessors = { a };
  let t = 0;
  for (const f of Object.values(accessors)) t += f();
  console.log(t);
  return <p>{a()}</p>;
}

// `for…of` over an array holder: the loop binding is called.
export function ForOfHolder() {
  const [a] = createSignal(1);
  const [b] = createSignal(2);
  const sources = [a, b];
  let t = 0;
  for (const f of sources) t += f();
  console.log(t);
  return <p>{a()}</p>;
}

// Twin: the loop binding is never called.
export function ForOfHeld() {
  const [a] = createSignal(1);
  const accessors = { a };
  let n = 0;
  for (const f of Object.values(accessors)) n += typeof f === "function" ? 1 : 0;
  console.log(n);
  return <p>{a()}</p>;
}

// A holder literal's getter reads its slot through `this`.
export function ThisGetter() {
  const [s] = createSignal(1);
  const o = { s, get v() { return this.s(); } };
  const x = o.v;
  console.log(x);
  return <p>{s()}</p>;
}

// A standard call reads a props root's getters.
function Child(props: { a: number }) {
  const x = JSON.stringify(props);
  console.log(x);
  return <i>x</i>;
}

export function HostRead() {
  const [s] = createSignal(1);
  return <div><Child a={s()} /></div>;
}

// An ambient function the program puts on the global object.
export function Ambient() {
  const [a] = createSignal(1);
  (globalThis as unknown as { ext: () => number }).ext = a;
  const x = ext();
  console.log(x);
  return <p>{a()}</p>;
}
