import { createEffect, createOptimisticStore } from "solid-js";

// ADR 1175: a user effect's first apply in `render`'s tail flush writes an optimistic store whose compute answers a fresh
// object literal of primitives (it runs no code) a microtask later at the earliest: rc.21 throws. A violation.
export function App() {
  const [, setO] = createOptimisticStore(async () => ({ v: 1 }), { v: 0 });
  createEffect(
    () => 1,
    () => {
      setO((draft) => {
        draft.v = 2;
      });
    },
  );
  return <div />;
}
