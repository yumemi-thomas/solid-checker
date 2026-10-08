import { getOwner, onCleanup, runWithOwner } from "solid-js";
import { throttle } from "@solid-primitives/scheduled";
export default function App() {
  const owner = getOwner();
  const done = document.createElement("p");
  done.id = "done";
  document.body.append(done);
  const trigger = throttle(() => { runWithOwner(owner, () => onCleanup(() => {})); done.textContent = "done"; }, 10);
  queueMicrotask(() => trigger());
  return document.createElement("p");
}
