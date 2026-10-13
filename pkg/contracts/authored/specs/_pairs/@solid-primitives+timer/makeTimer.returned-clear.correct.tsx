import { onCleanup } from "solid-js";
import { makeTimer } from "@solid-primitives/timer";
export default function App() {
  const done = document.createElement("p");
  done.id = "done";
  document.body.append(done);
  const stop = makeTimer(() => onCleanup(() => {}), 10, setTimeout);
  queueMicrotask(() => { stop(); });
  setTimeout(() => { done.textContent = "done"; }, 50);
  return document.createElement("p");
}
