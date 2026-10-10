import { createSignal } from "solid-js";
import { createReducer } from "@solid-primitives/memo";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createReducer((value: () => number) => value, () => {
      source();
      try { setSink(1); } catch { /* Preserve the owned write diagnostic and continue mounting. */ }
      return 1;
    });
  const finish = () => { setDone("done"); };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
