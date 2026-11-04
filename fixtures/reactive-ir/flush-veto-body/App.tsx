import { createEffect, createSignal, flush } from "solid-js";

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
  flush();
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
  flush();
  return <div>{seen()}</div>;
}

// A write in the cleanup the apply returns: a rerun runs it from the drain.
export function WritesInReturnedCleanup() {
  const [count] = createSignal(0);
  const [seen, setSeen] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      return () => {
        setSeen(1);
      };
    },
  );
  return <div>{seen()}</div>;
}
