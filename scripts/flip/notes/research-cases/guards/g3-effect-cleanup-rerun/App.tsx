import { createSignal, createEffect, onCleanup } from "solid-js";
const [c] = createSignal(1);
const [on, setOn] = createSignal(1);
export function App() {
  createEffect(() => on(), () => () => { c(); });
  return <p />;
}
export async function after() { setOn(2); }
