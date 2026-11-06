import { createMemo, createSignal } from "solid-js";
export function BodyWrite() {
  const [level] = createSignal(1);
  const doubled = createMemo(() => level() * 2);
  return <p>{doubled()}</p>;
}
