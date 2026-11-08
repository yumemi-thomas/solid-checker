import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

export function App() {
  setTimeout(() => { setW(1); }, 0);
  Promise.resolve().then(() => { setW(2); });
  return <p />;
}
