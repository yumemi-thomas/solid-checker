import { createSignal } from "solid-js";
import { createDate } from "@solid-primitives/date";
export default function App() {
  const [source] = createSignal(0);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  const [value] = createDate(() => {
    const value = source();
    return value;
  });
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
