import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
export function App() {
  const p = Promise.resolve();
  (p as Promise<void>).then(() => { setW(1); });
  return <p />;
}
