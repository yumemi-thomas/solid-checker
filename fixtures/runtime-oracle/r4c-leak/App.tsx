import { createSignal, createMemo, createEffect, onCleanup, createStore } from "solid-js";
import { handlers, inMemo } from "./lib";

// writes a signal in an event (legal) and in a memo (illegal) via flow
export function WriteEvent() {
  const [count, setCount] = createSignal(0);
  const h = handlers(() => setCount(count() + 1));
  return <button {...h}>a</button>;
}
export function WriteTracked() {
  const [n, setN] = createSignal(0);
  const run = inMemo(() => setN(1));
  const m = createMemo(() => (run(), n()));
  return <div>{m()}</div>;
}
// creates owners inside the flowed callback
export function OwnerInEvent() {
  const [count] = createSignal(0);
  const h = handlers(() => {
    createEffect(() => count(), () => undefined);
    onCleanup(() => undefined);
  });
  return <button {...h}>b</button>;
}
export function OwnerInTracked() {
  const [count] = createSignal(0);
  const run = inMemo(() => {
    createEffect(() => count(), () => undefined);
  });
  const m = createMemo(() => (run(), 1));
  return <div>{m()}</div>;
}
// async read
export function AsyncEvent() {
  const data = createMemo(async () => 1);
  const h = handlers(() => console.log(data()));
  return <button {...h}>c</button>;
}
export function AsyncTracked() {
  const data = createMemo(async () => 1);
  const run = inMemo(() => console.log(data()));
  const m = createMemo(() => (run(), 1));
  return <div>{m()}</div>;
}
// store write and props destructure
export function StoreWrite() {
  const [s, set] = createStore({ a: 1 });
  const run = inMemo(() => set((d) => { d.a = s.a + 1; }));
  const m = createMemo(() => (run(), 1));
  return <div>{m()}</div>;
}
export function Destructure(props: { v: number }) {
  const run = inMemo(() => {
    const { v } = props;
    console.log(v);
  });
  const m = createMemo(() => (run(), 1));
  return <div>{m()}</div>;
}
