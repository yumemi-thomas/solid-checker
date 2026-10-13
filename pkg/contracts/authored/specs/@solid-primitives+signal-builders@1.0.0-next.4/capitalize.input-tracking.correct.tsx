import { createSignal } from "solid-js";
import { capitalize } from "@solid-primitives/signal-builders";
export default function App() {
  const [source] = createSignal("hello");
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  const result = capitalize(() => {
    const value = source();
    return value;
  });
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
