import { createSignal } from "solid-js";
import { createDerivedStaticStore } from "@solid-primitives/static-store";
export default function App() {
  const [source] = createSignal(0);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  const state = createDerivedStaticStore(() => ({ value: (() => {
    const value = source();
    try { setSink(1); } catch { /* Expected REACTIVE_WRITE_IN_OWNED_SCOPE. */ }
    return value;
  })() }));
  const finish = () => {
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
