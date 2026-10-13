import { createSignal, getOwner, onCleanup, runWithOwner } from "solid-js";
import { createShortcut } from "@solid-primitives/keyboard";
export default function App() {
  const owner = getOwner();
  const [done, setDone] = createSignal("");
  createShortcut(["A"], () => {
    runWithOwner(owner, () => onCleanup(() => {}));
    setDone("done");
  });
  const dispatch = () => setTimeout(() => {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "a", repeat: false }));
    window.dispatchEvent(new KeyboardEvent("keyup", { key: "a" }));
  }, 0);
  return <><button id="target" onClick={dispatch}>dispatch</button><p id="done">{done()}</p></>;
}
