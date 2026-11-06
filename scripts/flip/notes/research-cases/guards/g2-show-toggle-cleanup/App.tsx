import { createSignal, onCleanup, Show } from "solid-js";
const [c] = createSignal(1);
const [on, setOn] = createSignal(true);
function Inner() { onCleanup(() => { c(); }); return <i />; }
export function App() {
  return <Show when={on()}><Inner /></Show>;
}
export async function after() { setOn(false); }
