// Positive control: the read after the declarator (STRICT_READ_UNTRACKED).
import { createSignal } from "solid-js";

export default function App() {
  const [r] = createSignal(0);
  [1].forEach(() => r());
  return <span id="w">{r()}</span>;
}
