import { createSignal, createRoot, onCleanup } from "solid-js";
const [w, setW] = createSignal(0);
function Inner() { setW(1); return <i />; }
export function App() {
  const dispose = createRoot((d) => { onCleanup(() => { <Inner />; }); return d; });
  return <button ref={(b) => { (globalThis as any).__b = b; }} onClick={() => { dispose(); }} />;
}
export async function after() { (globalThis as any).__b.click(); }
