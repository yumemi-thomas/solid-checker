import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
function coerce<T>(x: unknown): T { return x as T; }
export function App() {
  const p = coerce<Promise<void>>({ then(f: () => void) { f(); } });
  p.then(() => { setW(1); });
  return <p>{w()}</p>;
}
