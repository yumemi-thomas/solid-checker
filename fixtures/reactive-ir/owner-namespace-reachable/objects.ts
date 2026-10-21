import { createEffect, createSignal, runWithOwner } from "solid-js";

const [count] = createSignal(0);

// The module's namespace object is handed out whole (App.tsx passes it to
// `Object.entries`), so every export of this module has an unknown caller.
export function viaObject() {
  return runWithOwner(null, () => {
    createEffect(count, () => {});
    return 1;
  });
}
