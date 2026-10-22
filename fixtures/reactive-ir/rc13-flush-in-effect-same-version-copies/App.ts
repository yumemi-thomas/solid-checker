import { createEffect, createRoot, createSignal, flush } from "solid-js";
import { commit } from "./lib/helper";

createRoot(() => {
  const [value] = createSignal(1);
  // Positive: this file's `flush` and `createEffect` are the same copy.
  createEffect(value, () => {
    flush();
  });
  // Negative: `commit` loads the copy under lib/node_modules, a second
  // installed `solid-js` at the same version, with a queue of its own.
  createEffect(value, () => {
    commit();
  });
});
