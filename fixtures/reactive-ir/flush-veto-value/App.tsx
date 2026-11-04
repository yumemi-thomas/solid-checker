import { createEffect, createSignal, flush } from "solid-js";

// The flush function handed to a project helper that runs it in place: a
// reference that is not a call of the primitive is not followed.
function run(task: () => void) {
  task();
}

// A computation created on the apply's stack: SC6002 and SC4001 rest on the
// apply running with no owner.
export function CreatesInApply() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      createEffect(
        () => 1,
        () => {},
      );
    },
  );
  run(flush);
  return <div />;
}

// A write in the apply: SC2001 is silent because the apply runs with no owner.
export function WritesInApply() {
  const [count] = createSignal(0);
  const [seen, setSeen] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      setSeen(1);
    },
  );
  run(flush);
  return <div>{seen()}</div>;
}
