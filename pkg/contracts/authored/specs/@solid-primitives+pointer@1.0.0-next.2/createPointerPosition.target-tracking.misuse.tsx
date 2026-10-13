import { createSignal } from "solid-js";
import { createPointerPosition } from "@solid-primitives/pointer";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createPointerPosition({
    target: () => {
      source();
      try { setSink(1); } catch { /* Preserve REACTIVE_WRITE_IN_OWNED_SCOPE and continue mounting. */ }
      return document.body;
    }
  });
  const finish = () => {
    // The misuse write belongs to the tracked target callback.
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
