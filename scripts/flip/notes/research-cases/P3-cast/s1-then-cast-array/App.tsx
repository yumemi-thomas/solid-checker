import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
export function App() {
  const t = { forEach(f: () => void) { queueMicrotask(f); } };
  (t as unknown as number[]).forEach(() => { w(); });
  return <p>{w()}</p>;
}
