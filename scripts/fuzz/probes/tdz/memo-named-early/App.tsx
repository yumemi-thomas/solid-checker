// open-work: a callback named in a nested function run before its declarator.
import { createMemo, createSignal } from "solid-js";

export default function App() {
  const [v, setV] = createSignal(0);
  const early = () => createMemo(write);
  early();
  const write = () => { setV(1); return 1; };
  return <span id="w">{v()}</span>;
}
