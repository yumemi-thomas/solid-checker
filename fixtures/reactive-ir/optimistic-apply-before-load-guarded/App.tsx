import { createEffect, createOptimistic } from "solid-js";

// The setter is not on every completion of the apply's first run (Chrome is silent): uncertifiable.
export function App() {
  const [, setO] = createOptimistic(async () => 1);
  createEffect(
    () => 1,
    (v) => {
      if (v > 1) setO(2);
    },
  );
  return <div />;
}
