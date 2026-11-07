import { createMemo, createSignal } from "solid-js";

const late = (cb: () => void) => { queueMicrotask(cb); };
const held: any = {};
held[Symbol("forEach").description!] = late;

const [items] = createSignal<number[]>([1]);

export function App() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    items().forEach(() => { set(1); });
    return s();
  });
  return <p>{m()}</p>;
}
