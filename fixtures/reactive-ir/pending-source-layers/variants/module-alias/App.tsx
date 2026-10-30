import { createRenderEffect } from "solid-js";
import { remote } from "./source";

// Violation (SC5001): a `const` alias of the imported module-scope memo,
// declared before the effect, is that memo, pending at the first run.
export function ModuleAlias() {
  const read = remote;
  createRenderEffect(
    () => 1,
    () => {
      read();
    },
  );
  return <p>static</p>;
}
