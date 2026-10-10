import { createRoot } from "solid-js";
import { createRootPool } from "@solid-primitives/rootless";
createRoot(dispose => {
  createRootPool(() => 1);
  return dispose;
});
