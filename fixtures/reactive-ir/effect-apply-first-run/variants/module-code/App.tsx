import { createEffect, createMemo, createRenderEffect, createRoot } from "solid-js";

// The module's scope creates a root whose render effect goes pending when the
// module is imported, before the render runs.
createRoot(() => {
  const other = createMemo(async () => 2);
  createRenderEffect(other, () => {});
});

// Uncertifiable (SC1001 stays): a module scope runs code.
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
