import { createRoot } from "solid-js";
import { createPointerListeners } from "@solid-primitives/pointer";

createRoot(dispose => {
  createPointerListeners({ target: document.body, onDown: event => console.log(event.x) });
  return dispose;
});
