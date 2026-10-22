// Re-exports the copy under node_modules, the one this directory loads.
import { createEffect as localEffect } from "solid-js";
export { localEffect as effectOfThisCopy };
export { createRoot as rootOfThisCopy } from "solid-js";
