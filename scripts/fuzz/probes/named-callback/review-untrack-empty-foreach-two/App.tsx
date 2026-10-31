// FP candidate: named_callback_write_role climbs a zero-run library callback; two untrack positions.
import { createSignal, untrack } from "solid-js";
export default function App() {
  const [w, setW] = createSignal(0);
  const none: number[] = [];
  const run = () => { none.forEach(() => { setW(7); }); };
  untrack(run);
  untrack(run);
  return <span id="w">{w()}</span>;
}
