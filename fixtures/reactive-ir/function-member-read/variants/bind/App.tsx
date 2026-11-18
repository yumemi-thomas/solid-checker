import { createRoot } from "solid-js";
import { native } from "fnbindpkg";

// Uncertifiable: `Function.bind.apply(fn, a)` constructs code when `fn` is `Function` (`code-generation`).
createRoot(() => {
  native(() => {});
});
