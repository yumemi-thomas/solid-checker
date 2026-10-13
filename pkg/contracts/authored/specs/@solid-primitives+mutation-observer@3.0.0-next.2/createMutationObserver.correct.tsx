import { createRoot } from "solid-js";
import { createMutationObserver } from "@solid-primitives/mutation-observer";
createRoot(dispose => {
  createMutationObserver(document.body, { childList: true }, () => {});
  return dispose;
});
