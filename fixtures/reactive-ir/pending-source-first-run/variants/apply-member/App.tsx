import { createRenderEffect } from "solid-js";
import { remote } from "./source";

// Uncertifiable (ADR 0510 review): the apply reads a member of `undefined`
// before the read, so its first run throws a TypeError first.
export function Module() {
  createRenderEffect(
    () => 1,
    (v) => {
      (v as any).a.b;
      remote();
    },
  );
  return <p>static</p>;
}
