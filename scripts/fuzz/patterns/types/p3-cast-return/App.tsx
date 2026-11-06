import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
const mk = () => ({ then(f: () => void) { f(); } }) as unknown as Promise<void>;
export function App() {
  mk().then(() => { setW(1); });
  return <p />;
}
