import { createSignal, createRoot, onCleanup } from "solid-js";
const [c] = createSignal(1);
export function App() {
  const dispose = createRoot((d) => { onCleanup(() => { c(); }); return d; });
  return <button onClick={() => { dispose(); }} />;
}
export async function after() { document.querySelector("button")!.click(); }
