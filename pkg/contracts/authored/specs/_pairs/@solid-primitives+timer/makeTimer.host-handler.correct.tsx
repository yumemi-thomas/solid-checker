import { createSignal, getOwner, onCleanup, runWithOwner } from "solid-js";
import { makeTimer } from "@solid-primitives/timer";
export default function App() {
  const owner = getOwner();
  const [done, setDone] = createSignal("");
  const handler = () => {
    runWithOwner(owner, () => onCleanup(() => {}));
    setDone("done");
  };
  const clear = makeTimer(handler, 10, setTimeout);
  onCleanup(clear);
  return <p id="done">{done()}</p>;
}
