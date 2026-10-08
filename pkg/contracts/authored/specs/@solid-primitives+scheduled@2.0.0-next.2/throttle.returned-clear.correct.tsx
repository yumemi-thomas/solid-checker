import { onCleanup } from "solid-js";
import { throttle } from "@solid-primitives/scheduled";
export default function App() {
  const done = document.createElement("p");
  done.id = "done";
  document.body.append(done);
  const trigger = throttle(() => onCleanup(() => {}), 10);
  queueMicrotask(() => { trigger(); trigger.clear(); });
  setTimeout(() => { done.textContent = "done"; }, 50);
  return document.createElement("p");
}
