import { createSignal } from "solid-js";
import { createEventSignal } from "@solid-primitives/event-listener";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createEventSignal(window, () => {
      source();
      // The callback only reads source.
      return "click" as const;
    });
  const finish = () => { setSink(1); setDone("done"); };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
