import { createRoot } from "solid-js";
import { createVideoFrameCallback } from "@solid-primitives/video";
createRoot(dispose => {
  createVideoFrameCallback(() => undefined, () => {});
  return dispose;
});
