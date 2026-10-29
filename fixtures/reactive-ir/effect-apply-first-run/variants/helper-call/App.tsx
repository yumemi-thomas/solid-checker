import { createEffect, createMemo, createRenderEffect } from "solid-js";

function setup() {
  const other = createMemo(async () => 2);
  createRenderEffect(other, () => {});
}

// Uncertifiable (SC1001 stays): the body calls a helper, which may create a
// pending render effect.
export function C() {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  setup();
  return <div />;
}
