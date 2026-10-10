import { createSignal, getOwner, onCleanup, runWithOwner } from "solid-js";
import { onElementConnect } from "@solid-primitives/lifecycle";
export default function App() {
  const owner = getOwner();
  const [done, setDone] = createSignal("");
  const element = document.createElement("div");
  element.style.width = "20px";
  element.style.height = "20px";
  onCleanup(() => element.remove());
  onElementConnect(element, () => {
    runWithOwner(owner, () => onCleanup(() => {}));
    setDone("done");
  });
  return <><button id="target" onClick={() => { document.body.append(element); }}>attach</button><p id="done">{done()}</p></>;
}
