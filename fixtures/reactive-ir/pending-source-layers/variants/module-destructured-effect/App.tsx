import { createEffect } from "solid-js";
import { remote } from "./source";

// Violation (SC5001): the imported memo taken out of an inline object
// literal; the first render's tail flush runs the user effect while it is
// pending.
export function ModuleAlias() {
  const { read } = { read: remote };
  createEffect(
    () => 1,
    () => {
      read();
    },
  );
  return <p>static</p>;
}
