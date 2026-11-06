import { createSignal, createRoot } from "solid-js";
const [w, setW] = createSignal(0);
function Inner() { setW(1); return <i />; }
export function App() {
  return <button ref={(b) => { (globalThis as any).__b = b; }} onClick={() => { createRoot(() => <Inner />); }} />;
}
export async function after() { (globalThis as any).__b.click(); }
