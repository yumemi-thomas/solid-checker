import { createSignal } from "solid-js";
import { round } from "@solid-primitives/signal-builders";
export default function App() {
  const [source] = createSignal(1.2);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  const result = round(() => {
    const value = source();
    try { setSink(1); } catch { /* Expected REACTIVE_WRITE_IN_OWNED_SCOPE. */ }
    return value;
  });
  const finish = () => {
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
