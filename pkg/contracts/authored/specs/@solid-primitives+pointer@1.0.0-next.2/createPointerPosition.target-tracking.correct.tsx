import { createSignal } from "solid-js";
import { createPointerPosition } from "@solid-primitives/pointer";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  createPointerPosition({
    target: () => {
      source();
      // The write runs only in the event handler below.
      return document.body;
    }
  });
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
