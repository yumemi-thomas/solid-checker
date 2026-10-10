import { createSignal } from "solid-js";
import { createReducer } from "@solid-primitives/memo";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createReducer((value: () => number) => value, () => {
      source();
      // The callback only reads source.
      return 1;
    });
  const finish = () => { setSink(1); setDone("done"); };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
