import { createRoot } from "solid-js";
import { helper } from "./barrel-b";

// b.ts is the entry: it evaluates c.tsx before d.ts, so `helper` is still
// uninitialized here: ReferenceError.
createRoot(() => {
  helper();
});
