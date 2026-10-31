// Positive control: the library callback runs once (REACTIVE_WRITE_IN_OWNED_SCOPE expected).
import { createSignal, untrack } from "solid-js";
export default function App() {
  const [w, setW] = createSignal(0);
  const run = () => { [1].forEach(() => { setW(7); }); };
  untrack(run);
  untrack(run);
  return <span id="w">{w()}</span>;
}
