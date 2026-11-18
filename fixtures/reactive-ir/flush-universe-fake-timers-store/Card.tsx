import { action, createEffect, createSignal, createTrackedEffect } from "solid-js";

const [n, setN] = createSignal(0);

// The program steps an action, runs an effect's apply and opens a leaf scope: no program-wide reason holds, so each flush in the test file stands on the
// census class of its own caller.
export const step = action(function* () {
  setN(1);
  yield 1;
});

export function Card() {
  createEffect(() => n(), () => {});
  createTrackedEffect(() => {
    n();
  });
  return <div />;
}
