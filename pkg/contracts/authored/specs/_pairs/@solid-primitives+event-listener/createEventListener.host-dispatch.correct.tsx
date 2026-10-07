import { createSignal, getOwner, onCleanup, runWithOwner } from "solid-js";
import { createEventListener } from "@solid-primitives/event-listener";
export default function App() {
  const owner = getOwner();
  const [done, setDone] = createSignal("");
  const target = new EventTarget();
  const handler = () => {
    runWithOwner(owner, () => onCleanup(() => {}));
    setDone("done");
  };
  createEventListener(target, "batch1-host", handler);
  const dispatch = () => setTimeout(() => target.dispatchEvent(new Event("batch1-host")), 0);
  return <><button id="target" onClick={dispatch}>dispatch</button><p id="done">{done()}</p></>;
}
