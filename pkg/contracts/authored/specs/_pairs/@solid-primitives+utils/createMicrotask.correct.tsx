import { createRoot } from "solid-js";
import { createMicrotask } from "@solid-primitives/utils";

createRoot(dispose => {
  createMicrotask(() => console.log("tick"));
  return dispose;
});
