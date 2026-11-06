import { createSignal, onCleanup, For } from "solid-js";
const [c] = createSignal(1);
const [xs, setXs] = createSignal([1, 2]);
function Row() { onCleanup(() => { c(); }); return <i />; }
export function App() {
  return <For each={xs()}>{() => <Row />}</For>;
}
export async function after() { setXs([1]); }
