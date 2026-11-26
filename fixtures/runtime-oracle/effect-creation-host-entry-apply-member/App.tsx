import { createEffect, createMemo, untrack } from "solid-js";
const sched = { setTimeout: (fn: () => void) => fn() };

export function ApplyMember() {
  createEffect(
    () => 0,
    () => {
      sched.setTimeout(() => {
        createMemo(() => 5);
      });
    },
  );
  return <p>m</p>;
}

