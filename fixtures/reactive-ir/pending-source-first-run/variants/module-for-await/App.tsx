import { createRenderEffect } from "solid-js";
import { remote } from "./source";

export function Module() {
  createRenderEffect(
    () => 1,
    () => {
      remote();
    },
  );
  return <p>static</p>;
}
