// open-work (ADR 0464 residual): r() throws before count() runs.
import { createSignal } from "solid-js";

export default function App() {
  const [count] = createSignal(0);
  [1].forEach(() => { r(); count(); });
  const [r] = createSignal(0);
  return <span id="w">{r()}{count()}</span>;
}
