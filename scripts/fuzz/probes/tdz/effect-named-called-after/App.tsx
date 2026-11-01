import { createEffect, createSignal } from "solid-js";

export default function App() {
  const [v] = createSignal(0);
  const early = () => createEffect(() => 1, read);
  const read = () => { v(); };
  early();
  return <span id="w">{v()}</span>;
}
