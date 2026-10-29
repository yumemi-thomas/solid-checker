import { createEffect, createMemo } from "solid-js";

// Uncertifiable (SC1001 stays): the read is not on every completion of the
// first run of the closure the apply calls (an early return on the first
// value), so the first run never reads the pending source.
export function C() {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    (v) => {
      // The guard sits in a closure the apply calls.
      const step = () => {
        if (v === 1) return;
        value();
      };
      step();
    },
  );
  return <div />;
}
