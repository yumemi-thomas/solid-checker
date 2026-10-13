import { createRoot } from "solid-js";
import { createPolled } from "@solid-primitives/timer";

createRoot(dispose => {
  createPolled(() => Date.now(), 1000);
  return dispose;
});
