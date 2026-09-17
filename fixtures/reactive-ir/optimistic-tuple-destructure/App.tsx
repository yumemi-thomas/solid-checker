// The control half: **declaration** destructuring of a reactive tuple.
//
// `const [count] = createOptimistic(0)` reaches source discovery through the
// binding path, whose guard is `Dialect::creates_reactive_source` — a question
// that has always named the optimistic primitives. Nothing here depends on
// `returns_reactive_tuple`, and dropping the optimistic entries from that list
// leaves all three of these findings in place. That is the point: it is what
// makes `Assign.tsx` next door mean something specific rather than "some
// destructure somewhere stopped working".
//
// Each function reads the same source twice, in the compute (tracked, silent)
// and in the apply (not tracked, SC1001). Only the apply-phase reads are
// expected.
import { createEffect, createOptimistic, createOptimisticStore, createSignal } from "solid-js";

export function OptimisticAccessor() {
  const [count] = createOptimistic(0);
  createEffect(
    () => count(),
    () => {
      console.log(count());
    },
  );
  return <span />;
}

export function OptimisticStorePath() {
  const [row] = createOptimisticStore({ label: "a" });
  createEffect(
    () => row.label,
    () => {
      console.log(row.label);
    },
  );
  return <span />;
}

export function SignalControl() {
  const [name] = createSignal("a");
  createEffect(
    () => name(),
    () => {
      console.log(name());
    },
  );
  return <span />;
}
