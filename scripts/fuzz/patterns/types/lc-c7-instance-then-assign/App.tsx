import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
export function App() {
  const p = Promise.resolve();
  Object.assign(p, { then: (f: () => void) => { f(); } });
  p.then(() => { setW(1); });
  return <p>{w()}</p>;
}
