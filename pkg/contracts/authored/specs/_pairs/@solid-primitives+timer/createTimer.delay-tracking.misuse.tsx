import { createMemo, createSignal } from "solid-js";
import { createTimer } from "@solid-primitives/timer";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createTimer(() => {}, () => {
    const value = source();
    try { setSink(1); } catch { /* Keep the emitted write diagnostic; allow mount to settle. */ }
    return 60_000 + value;
  }, setInterval);
  const finish = () => {
    // The write belongs to the misuse's owned callback above.
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
