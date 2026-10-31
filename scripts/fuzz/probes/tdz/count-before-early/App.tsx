// Distinguishing: count() runs before the early mention throws (STRICT_READ_UNTRACKED at count()).
import { createSignal } from "solid-js";

export default function App() {
  const [count] = createSignal(0);
  [1].forEach(() => { count(); r(); });
  const [r] = createSignal(0);
  return <span id="w">{r()}{count()}</span>;
}
