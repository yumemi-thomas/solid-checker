import { createEffect, createMemo, createSignal } from "solid-js";

// Violation (SC5001): the source reaches the apply through a tuple taken
// apart in the body and a closure the apply calls; an inert local function
// and a literal signal run in the body.
export function C() {
  const value = createMemo(async () => 1);
  createSignal(0);
  const pair = [0, value] as const;
  const [, read] = pair;
  const make = () => ({ read });
  const { read: again } = make();
  createEffect(
    () => 1,
    () => {
      const step = () => {
        again();
      };
      step();
    },
  );
  return <div>text</div>;
}
