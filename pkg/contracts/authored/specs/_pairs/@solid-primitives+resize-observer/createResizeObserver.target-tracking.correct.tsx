import { createMemo, createSignal } from "solid-js";
import { createResizeObserver } from "@solid-primitives/resize-observer";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createResizeObserver(() => {
    const value = source();
    // Write to sink only in the event handler below.
    return document.body;
  }, () => {});
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
