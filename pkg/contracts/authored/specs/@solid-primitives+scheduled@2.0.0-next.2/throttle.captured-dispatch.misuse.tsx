import { getOwner, onCleanup, runWithOwner } from "solid-js";
import { throttle } from "@solid-primitives/scheduled";
export default function App() {
  const owner = getOwner();
  const done = document.createElement("p");
  done.id = "done";
  document.body.append(done);
  const callback = () => { onCleanup(() => {}); done.textContent = "done"; };
  const trigger = throttle(callback, 10);
  trigger();
  return document.createElement("p");
}
