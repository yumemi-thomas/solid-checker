import { createSignal } from "solid-js";
import { capitalize } from "@solid-primitives/signal-builders";
export default function App() {
  const [source] = createSignal("hello");
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  let runs = 0;
  capitalize(() => {
    runs++;
    const value = source();
    return value;
  });
  if (runs < 1) throw new Error("mandatory first compute did not run");
  const finish = () => { setSink(1); setDone("done"); };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
