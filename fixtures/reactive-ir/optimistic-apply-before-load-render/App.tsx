import { createOptimisticStore, createRenderEffect } from "solid-js";

// ADR 1175: a render effect's apply runs at its creation, in `render`'s pass, and the store's seed is no answer: rc.21 throws
// OPTIMISTIC_WRITE_BEFORE_LOAD at the setter. A violation.
export function App() {
  const [, setO] = createOptimisticStore(async () => ({ v: 1 }), { v: 0 });
  createRenderEffect(
    () => 1,
    () => {
      setO((draft) => {
        draft.v = 2;
      });
    },
  );
  return <div />;
}
