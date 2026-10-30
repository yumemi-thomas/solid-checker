import { createRenderEffect } from "solid-js";
import { remote } from "./source";

// Uncertifiable (ADR 0510 review): `warm.ts`, a root no project file imports,
// also loads the source; a page that runs it before the entry settles the
// memo in the microtask checkpoint between the two module scripts.
export function Module() {
  createRenderEffect(
    () => 1,
    () => {
      remote();
    },
  );
  return <p>static</p>;
}
