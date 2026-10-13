import { createSignal } from "solid-js";
import { createEventSignal } from "@solid-primitives/event-listener";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createEventSignal(() => {
      source();
      try { setSink(1); } catch { /* Preserve the owned write diagnostic and continue mounting. */ }
      return window;
    }, "click");
  const finish = () => { setDone("done"); };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
