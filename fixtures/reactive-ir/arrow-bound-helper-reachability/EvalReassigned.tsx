// Negative: a direct `eval` rebinds `go` before the call, so the call does
// not reach this declaration.
import { createSignal } from "solid-js";

const [, setCount] = createSignal(0);
function go() {
  setCount(1);
}
export function App() {
  eval("go = () => {}");
  go();
  return <button />;
}
