import { createEffect, createMemo } from "solid-js";

// Uncertifiable (SC1001 stays): the rendered expression is a render effect
// that may go pending and hold the flush.
export function C() {
  const value = createMemo(async () => 1);
  const label = createMemo(async () => "a");
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  return <div>{label()}</div>;
}
