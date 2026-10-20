import { createSignal } from "solid-js";
import * as L from "./lib";

export function Destructure() {
  const [count] = createSignal(0);
  const { onKey } = L.handlers(() => console.log(count()));
  onKey();
  return <div>a</div>;
}
export function OptionalCall() {
  const [count] = createSignal(0);
  const run = L.retain(() => console.log(count()));
  run?.();
  return <div onClick={run}>b</div>;
}
export function ArrayEach() {
  const [count] = createSignal(0);
  const run = L.retain(() => console.log(count()));
  [run].forEach((f) => f());
  return <div onClick={run}>c</div>;
}
export function Loop() {
  const [count] = createSignal(0);
  return <div {...L.loopy(() => console.log(count()))}>d</div>;
}
export function Helper() {
  const [count] = createSignal(0);
  return <div {...L.viaHelper(() => console.log(count()))}>e</div>;
}
export function Try() {
  const [count] = createSignal(0);
  return <div {...L.viaTry(() => console.log(count()))}>f</div>;
}
export function Memo() {
  const [count] = createSignal(0);
  const m = L.inMemo(() => console.log(count()));
  return <div>{m()}</div>;
}
export function Cleanup() {
  const [count] = createSignal(0);
  L.inCleanup(() => console.log(count()));
  return <div>h</div>;
}
export function Settled() {
  const [count] = createSignal(0);
  L.inSettled(() => console.log(count()));
  return <div>i</div>;
}
export function Tracked() {
  const [count] = createSignal(0);
  L.inTracked(() => console.log(count()));
  return <div>j</div>;
}
export function Apply() {
  const [count] = createSignal(0);
  L.inApply(() => console.log(count()));
  return <div>k</div>;
}
export function Untrack() {
  const [count] = createSignal(0);
  L.inUntrack(() => console.log(count()));
  return <div>l</div>;
}
export function Iife() {
  const [count] = createSignal(0);
  return <div {...L.iife(() => console.log(count()))}>m</div>;
}
export function ObjectCall() {
  const [count] = createSignal(0);
  return <div {...L.viaObjectCall(() => console.log(count()))}>n</div>;
}
export function Timeout() {
  const [count] = createSignal(0);
  L.viaTimeoutLater(() => console.log(count()));
  return <div>o</div>;
}
export function Then() {
  const [count] = createSignal(0);
  L.sameTick(() => console.log(count()));
  return <div>p</div>;
}
