import { createSignal, getOwner, onCleanup, runWithOwner } from "solid-js";
import { createTimer } from "@solid-primitives/timer";
export default function App() {
  const owner = getOwner();
  const [done, setDone] = createSignal("");
  const handler = () => {
    onCleanup(() => {});
    setDone("done");
  };
  createTimer(handler, 10, setTimeout);
  return <p id="done">{done()}</p>;
}
