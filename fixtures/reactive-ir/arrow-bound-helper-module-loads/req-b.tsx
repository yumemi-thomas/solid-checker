// Negative: req-a.ts `require`s this module before it declares `helper`.
import { createRoot } from "solid-js";
import { helper } from "./req-a";

createRoot(() => {
  helper();
});
