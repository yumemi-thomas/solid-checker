import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
const doubled = createMemo(() => count() * 2);

export function Good() {
  return <div>{doubled()}</div>;
}

export function Bad() {
  const value = doubled();
  return <div>{value}</div>;
}
