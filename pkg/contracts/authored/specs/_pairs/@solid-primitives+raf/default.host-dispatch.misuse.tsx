import { createSignal, getOwner, onCleanup, runWithOwner } from "solid-js";
import createFrame from "@solid-primitives/raf";
export default function App() {
  const owner = getOwner();
  const [done, setDone] = createSignal("");
  const [, start, stop] = createFrame(() => {
    onCleanup(() => {});
    stop();
    setDone("done");
  });
  return <><button id="target" onClick={() => start()}>start</button><p id="done">{done()}</p></>;
}
