// Distinguishing: the nested function is written before the declarator but called after it (REACTIVE_WRITE_IN_OWNED_SCOPE).
import { createMemo, createSignal } from "solid-js";

export default function App() {
  const [v, setV] = createSignal(0);
  const early = () => createMemo(write);
  const write = () => { setV(1); return 1; };
  early();
  return <span id="w">{v()}</span>;
}
