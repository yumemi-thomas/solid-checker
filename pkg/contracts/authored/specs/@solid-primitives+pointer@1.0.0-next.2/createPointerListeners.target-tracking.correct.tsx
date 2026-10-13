import { createSignal } from "solid-js";
import { createPointerListeners } from "@solid-primitives/pointer";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createPointerListeners({ target: () => {
      source();
      // The callback only reads source.
      return document.body;
    }, onDown: () => {} });
  const finish = () => { setSink(1); setDone("done"); };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
