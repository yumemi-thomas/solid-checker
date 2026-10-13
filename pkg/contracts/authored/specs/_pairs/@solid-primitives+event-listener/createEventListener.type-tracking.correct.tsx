import { createMemo, createSignal } from "solid-js";
import { createEventListener } from "@solid-primitives/event-listener";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createEventListener(window, () => {
    const value = source();
    // Write to sink only in the event handler below.
    return "click" as const;
  }, () => {});
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
