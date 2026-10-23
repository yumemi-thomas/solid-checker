import { createEffect, createSignal } from "solid-js";

declare const options: { defer: boolean };

// Whether the apply runs is decided by `defer`, which the syntax cannot read:
// SC2001 uncertifiable, blocker `deferred-apply` (ADR 0302).
export function UnreadDefer() {
  const [count, setCount] = createSignal(0);
  createEffect(
    () => count(),
    (value) => {
      setCount(value + 1);
    },
    options,
  );
  return <div>{count()}</div>;
}

// Control: with no options the apply runs, and a write in it is legal.
export function NoOptions() {
  const [count, setCount] = createSignal(0);
  createEffect(
    () => count(),
    (value) => {
      setCount(value + 1);
    },
  );
  return <div>{count()}</div>;
}
