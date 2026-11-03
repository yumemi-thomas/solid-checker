import { createEffect, createSignal, flush } from "solid-js";

// A project helper that calls flush: its callers decide the owner, and the
// census does not follow them.
function drainNow() {
  flush();
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
  drainNow();
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
  drainNow();
  return <div>{seen()}</div>;
}
