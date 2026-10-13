import { onSettled } from "solid-js";
import { createPointerListeners } from "@solid-primitives/pointer";
export default function App() {
  onSettled(() => {
    createPointerListeners({ target: document.body, onDown: () => {}, ondown: undefined });
  });
  return <div />;
}
