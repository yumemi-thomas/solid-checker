import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
window.setTimeout = ((f: () => void) => { f(); return 0; }) as any;
export function App() {
  setTimeout(() => { setW(1); }, 0);
  return <p />;
}
