import { createSignal } from "solid-js";
export function BodyWrite() {
  const [count] = createSignal(0);
  const text = `${count}`;
  return <p>{text}</p>;
}
