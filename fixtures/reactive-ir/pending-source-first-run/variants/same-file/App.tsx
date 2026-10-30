import { createMemo, createRenderEffect } from "solid-js";

// A module-scope memo of the component's own module, which only creates.
const remote = createMemo(async () => 1);

// Violation (SC5001): the render effect's first run meets it pending.
export function SameFile() {
  createRenderEffect(
    () => 1,
    () => {
      remote();
    },
  );
  return <p>static</p>;
}
