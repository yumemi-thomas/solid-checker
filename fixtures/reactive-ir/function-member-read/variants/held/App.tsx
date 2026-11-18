import { createRoot } from "solid-js";
import { native } from "fnheldpkg";

// Uncertifiable: `Function.toString` handed on reaches `Function` through `.constructor` (`code-generation`).
createRoot(() => {
  native(() => {});
});
