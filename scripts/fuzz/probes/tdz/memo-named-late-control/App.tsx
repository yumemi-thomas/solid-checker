// Positive control: the named callback declared before the call.
import { createMemo, createSignal } from "solid-js";

export default function App() {
  const [v, setV] = createSignal(0);
  const write = () => { setV(1); return 1; };
  const early = () => createMemo(write);
  early();
  return <span id="w">{v()}</span>;
}
