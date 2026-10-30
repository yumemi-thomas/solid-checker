import { createEffect } from "solid-js";
import { remote } from "./source";

// Violation (SC5001): a user effect's first apply runs in the first render's
// tail flush, which nothing holds, and meets the module memo pending.
export function ModuleEffect() {
  createEffect(
    () => 1,
    () => {
      remote();
    },
  );
  return <p>static</p>;
}
