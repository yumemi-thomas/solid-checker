import { createEffect, createOptimistic } from "solid-js";

// ADR 1175: the effect's first apply runs in `render`'s tail flush, before the async compute's first answer: rc.21 throws
// OPTIMISTIC_WRITE_BEFORE_LOAD at the setter. A violation.
export function App() {
  const [, setO] = createOptimistic(async () => 1);
  createEffect(
    () => 1,
    () => {
      setO(2);
    },
  );
  return <div />;
}
