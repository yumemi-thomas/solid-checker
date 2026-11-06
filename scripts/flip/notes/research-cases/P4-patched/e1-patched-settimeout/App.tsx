import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
(globalThis as any).setTimeout = (f: () => void) => { f(); return 0; };
export function App() {
  setTimeout(() => { setW(1); }, 0);
  return <p>{w()}</p>;
}
