import { createSignal } from "solid-js";
import { createPointerListeners } from "@solid-primitives/pointer";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createPointerListeners({ target: () => {
      source();
      try { setSink(1); } catch { /* Preserve the owned write diagnostic and continue mounting. */ }
      return document.body;
    }, onDown: () => {} });
  const finish = () => { setDone("done"); };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
