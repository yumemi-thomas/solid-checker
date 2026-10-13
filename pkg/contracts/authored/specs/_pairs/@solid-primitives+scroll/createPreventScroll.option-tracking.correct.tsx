import { createMemo, createSignal } from "solid-js";
import { createPreventScroll } from "@solid-primitives/scroll";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createPreventScroll({ enabled: () => {
    const value = source();
    // Write to sink only in the event handler below.
    return false;
  } });
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
