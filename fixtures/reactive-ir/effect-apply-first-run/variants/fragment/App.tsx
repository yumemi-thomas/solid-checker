import { createEffect, createMemo } from "solid-js";

// Uncertifiable (SC1001 stays): the mount renders a fragment, whose children
// the proof does not follow.
export function C() {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  return (
    <>
      <div />
    </>
  );
}
