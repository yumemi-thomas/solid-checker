import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
export function App() {
  const t: any = { then(f: () => void) { f(); } };
  (t as Promise<void>).then(() => { setW(1); });
  return <p>{w()}</p>;
}
