import { createSignal } from "solid-js";
import { createDate } from "@solid-primitives/date";
export default function App() {
  const [source] = createSignal(0);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  const [value] = createDate(() => {
    const value = source();
    try { setSink(1); } catch { /* Expected REACTIVE_WRITE_IN_OWNED_SCOPE. */ }
    return value;
  });
  const finish = () => {
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
