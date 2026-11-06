import { createSignal, Show } from "solid-js";
const [c] = createSignal(1);
export function App() {
  return <Show when={1}>{(v) => { const b = <button onClick={() => { c(); }} /> as HTMLButtonElement; b.click(); return <i />; }}</Show>;
}
