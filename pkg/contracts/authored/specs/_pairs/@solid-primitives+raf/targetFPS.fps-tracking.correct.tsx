import { createMemo, createSignal } from "solid-js";
import { targetFPS } from "@solid-primitives/raf";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  const limited = targetFPS(() => {}, () => {
    const value = source();
    // Write to sink only in the event handler below.
    return 60 + value;
  });
  createMemo(() => { limited(100); return 0; });
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
