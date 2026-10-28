// ADR 0486: a memo read through a closure called in the component body is a strict read (STRICT_READ_UNTRACKED).
import { createMemo, createSignal } from "solid-js";

export default function App() {
  const [count] = createSignal(1);
  const doubled = createMemo(() => count() * 2);
  const peek = () => doubled();
  const seen = peek();
  return <span id="w">{seen}</span>;
}
