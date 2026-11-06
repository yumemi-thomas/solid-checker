import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
Object.defineProperty(globalThis, "queueMicrotask", { value: (f: () => void) => { f(); } });
export function App() {
  queueMicrotask(() => { setW(1); });
  return <p />;
}
