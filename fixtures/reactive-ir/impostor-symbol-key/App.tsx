import { createMemo, createSignal } from "solid-js";

export const made = {
  [Symbol("made")]: () => 0,
  [Symbol.for("registered")]: () => 0,
  [Symbol.iterator]: () => 0,
};

const [items] = createSignal<number[]>([1]);
const [pending] = createSignal(Promise.resolve());

export function App() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    items().forEach(() => { set(1); });
    pending().then(() => { set(2); });
    return s();
  });
  return <p>{m()}</p>;
}
