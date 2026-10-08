import { createRoot } from "solid-js";
import { createMarker } from "@solid-primitives/marker";
createRoot(dispose => {
  createMarker(() => document.createElement("mark"));
  return dispose;
});
