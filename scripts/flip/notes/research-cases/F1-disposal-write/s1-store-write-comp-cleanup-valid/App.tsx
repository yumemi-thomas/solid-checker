import { createStore, createRoot, onCleanup } from "solid-js";
const [s, setS] = createStore({ a: 1 });
function Inner() { setS((d) => { d.a = 2; }); return <i />; }
export function App() {
  const dispose = createRoot((d) => { onCleanup(() => { createRoot(() => <Inner />); }); return d; });
  return <button ref={(b) => { (globalThis as any).__b = b; }} onClick={() => { dispose(); }} />;
}
export async function after() { (globalThis as any).__b.click(); }
