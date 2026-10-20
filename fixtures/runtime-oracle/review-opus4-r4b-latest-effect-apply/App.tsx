import { createSignal, createEffect, latest } from "solid-js";
export function App() {
  const [n] = createSignal(1);
  const [m] = createSignal(2);
  const [out, setOut] = createSignal(0);
  createEffect(() => n(), (v) => { const x = latest(() => m()); document.getElementById("mk8")!.textContent = String(v + x); });
  return <p id="mk8">{out()}</p>;
}
