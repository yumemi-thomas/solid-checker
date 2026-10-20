// Negative: the same, with a module-local `declare function require` shim.
// Negative: req-a.ts `require`s this module before it declares `helper`.
import { createRoot } from "solid-js";
import { helper } from "./shim-a";

createRoot(() => {
  helper();
});
