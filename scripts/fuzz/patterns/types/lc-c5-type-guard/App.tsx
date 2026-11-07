import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
function isPromise(x: unknown): x is Promise<void> { return typeof x === "object" && x !== null && "then" in x; }
export function App() {
  const t: unknown = { then(f: () => void) { f(); } };
  if (isPromise(t)) t.then(() => { setW(1); });
  return <p>{w()}</p>;
}
