import { createSignal } from "solid-js";
import { createElementCursor } from "@solid-primitives/cursor";
export default function App() {
  const [source] = createSignal("pointer" as const);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createElementCursor(document.body, () => {
    const value = source();
    return value;
  });
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
