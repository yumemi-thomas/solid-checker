import { createEffect, createMemo, createRenderEffect, createRoot } from "solid-js";

export function prepare() {
  createRoot(() => {
    const other = createMemo(async () => 2);
    createRenderEffect(other, () => {});
  });
}

// Uncertifiable (SC1001 stays): the entry calls project code before the render.
export function C() {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  return <div />;
}
