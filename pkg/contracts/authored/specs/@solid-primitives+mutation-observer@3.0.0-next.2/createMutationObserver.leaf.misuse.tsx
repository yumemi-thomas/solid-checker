import { onSettled } from "solid-js";
import { createMutationObserver } from "@solid-primitives/mutation-observer";
export default function App() {
  onSettled(() => { createMutationObserver(document.body, { childList: true }, () => {}); });
  return document.createElement("p");
}
