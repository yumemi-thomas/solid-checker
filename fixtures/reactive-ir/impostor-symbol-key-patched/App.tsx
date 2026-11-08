import { createMemo, createSignal } from "solid-js";
import "./patch";

const held: any = {};
held[Symbol("k")] = (cb: () => void) => { queueMicrotask(cb); };

const [items] = createSignal<number[]>([1]);

export function App() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    items().forEach(() => { set(1); });
    return s();
  });
  return <p>{m()}</p>;
}
