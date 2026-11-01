// resolve() in a library callback that runs once inside a named memo compute (RESOLVE_IN_REACTIVE_SCOPE expected).
import { createMemo, resolve } from "solid-js";
export default function App() {
  const compute = () => { [1].forEach(() => { resolve(() => 1); }); return 1; };
  const m = createMemo(compute);
  return <span id="w">{m()}</span>;
}
