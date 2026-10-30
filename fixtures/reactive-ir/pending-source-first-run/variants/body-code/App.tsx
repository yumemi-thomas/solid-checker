import { createRenderEffect } from "solid-js";
import { remote } from "./source";

// Uncertifiable: the body runs code before the effect is created.
export function Module() {
  JSON.stringify(1);
  createRenderEffect(
    () => 1,
    () => {
      remote();
    },
  );
  return <p>static</p>;
}
