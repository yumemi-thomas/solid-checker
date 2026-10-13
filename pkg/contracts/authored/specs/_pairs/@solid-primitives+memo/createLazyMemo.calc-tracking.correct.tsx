import { createMemo, createSignal } from "solid-js";
import { createLazyMemo } from "@solid-primitives/memo";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  const result = createLazyMemo(() => {
    const value = source();
    // Write to sink only in the event handler below.
    return value;
  });
  createMemo(() => result());
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
