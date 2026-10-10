import { createSignal } from "solid-js";
import { createDateNow } from "@solid-primitives/date";
export default function App() {
  const [source] = createSignal(30_000);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  const [now] = createDateNow(() => {
    const value = source();
    return value;
  });
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
