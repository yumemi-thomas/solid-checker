import { createSignal, createRoot, createMemo, onCleanup } from "solid-js";
const [w, setW] = createSignal(0);
export function App() {
  const dispose = createRoot((d) => { onCleanup(() => { createRoot(() => { createMemo(() => { setW(1); return 1; })(); }); }); return d; });
  return <button ref={(b) => { (globalThis as any).__b = b; }} onClick={() => { dispose(); }} />;
}
export async function after() { (globalThis as any).__b.click(); }
