import { createEffect, createSignal } from "solid-js";

export default function App() {
  const [v] = createSignal(0);
  const early = () => createEffect(() => 1, read);
  early();
  const read = () => { v(); };
  return <span id="w">{v()}</span>;
}
