import { createSignal, createStore } from "solid-js";

const [count, setCount] = createSignal(0);
const [store] = createStore({ a: { v: 0 }, items: [1] });

// Clean (ADR 0203, 0934): the program is closed and nothing here calls these
// exports, so no caller runs their reads.
export function bump() {
  setCount(count() + 1);
}
export function peek() {
  return count();
}
export function readStore() {
  if (store.a) console.log(store.items.length);
}
export const arrow = () => count();
export function outer() {
  const inner = () => count();
  return inner();
}
// Capitalized but returning no JSX: no component by module surface (ADR 0445).
export function Shaped(props: { n: number }) {
  return [count(), props.n];
}
// A call here in a handler proves nothing of the unseen ones.
export function step() {
  console.log(count());
}
export function Uses() {
  return <button onClick={() => step()}>step</button>;
}

// Clean: a read past an await on every path runs on a later task, off every
// caller's stack (ADR 0323).
export async function later() {
  await Promise.resolve();
  return count();
}

// Clean: a local helper only a handler calls, a helper nothing references, and
// a component's tracked read.
function local() {
  console.log(count());
}
function dead() {
  console.log(count());
}
export function Local() {
  return <button onClick={() => local()}>{count()}</button>;
}
