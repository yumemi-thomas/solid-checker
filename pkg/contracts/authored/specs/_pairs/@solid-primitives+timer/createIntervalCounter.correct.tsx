/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { createIntervalCounter } from "@solid-primitives/timer";
createRoot(dispose => {
  createIntervalCounter(60_000);
  return dispose;
});
