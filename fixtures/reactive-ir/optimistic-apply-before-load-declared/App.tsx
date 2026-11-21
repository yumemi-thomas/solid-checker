import { createEffect, createOptimistic } from "solid-js";

// A declared `loadingValue` answers before the compute does: no throw (Chrome is silent), and no proof.
export function App() {
  const [, setO] = createOptimistic(async () => 1, { loadingValue: 0 });
  createEffect(
    () => 1,
    () => {
      setO(2);
    },
  );
  return <div />;
}
