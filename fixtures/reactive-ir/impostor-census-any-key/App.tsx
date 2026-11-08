import { createSignal } from "solid-js";

const [, setW] = createSignal(0);
const [items] = createSignal<number[]>([1]);
const [pending] = createSignal(Promise.resolve());

// A key no fact names: it may be `forEach` or `then`.
export function define(key: string) {
  return { [key]: () => {} };
}

export function App() {
  items().forEach(() => { setW(1); });
  pending().then(() => { setW(2); });
  [1, 2].forEach(() => { setW(3); });
  return <p />;
}
