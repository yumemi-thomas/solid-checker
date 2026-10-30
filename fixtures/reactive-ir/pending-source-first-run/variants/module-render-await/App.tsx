import { createRenderEffect } from "solid-js";
import { remote } from "./source";

export function Module() {
  createRenderEffect(
    () => remote(),
    () => {},
  );
  return <p>static</p>;
}
