import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
export function App() {
  const h = { p: { then(f: () => void) { f(); } } } as unknown as { p: Promise<void> };
  h.p.then(() => { setW(1); });
  return <p>{w()}</p>;
}
