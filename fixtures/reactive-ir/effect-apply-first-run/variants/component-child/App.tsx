import { createEffect, createMemo, createRenderEffect } from "solid-js";

// A child whose render effect goes pending holds the flush.
function Child() {
  const other = createMemo(async () => 2);
  createRenderEffect(other, () => {});
  return <span />;
}

// Uncertifiable (SC1001 stays): the mount renders a component.
export function C() {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  return (
    <div>
      <Child />
    </div>
  );
}
