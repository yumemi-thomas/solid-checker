import { createRenderEffect } from "solid-js";
import { remote } from "./source";

// Uncertifiable: the alias is declared after the effect, whose first run
// meets it in its temporal dead zone (a ReferenceError, not the pending read).
export function ModuleAlias() {
  createRenderEffect(
    () => 1,
    () => {
      read();
    },
  );
  const read = remote;
  return <p>static</p>;
}
