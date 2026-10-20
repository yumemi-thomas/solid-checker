import { createSignal } from "solid-js";

function directFactory(): () => number {
  const [count] = createSignal(1);
  return (() => count()) as () => number;
}

export function DirectBody() {
  const read = directFactory();
  const value = read();
  return <span>{value}</span>;
}

export function DirectTracked() {
  const read = directFactory();
  return <span>{read()}</span>;
}

