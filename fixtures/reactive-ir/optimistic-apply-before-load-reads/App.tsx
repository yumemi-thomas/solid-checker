import { createEffect, createOptimistic } from "solid-js";

// The compute reads the pending source, so the apply waits for its answer (Chrome is silent): uncertifiable. A local
// `createOptimistic` that shadows the import is no optimistic creation.
export function App() {
  const [o, setO] = createOptimistic(async () => 1);
  createEffect(
    () => o(),
    () => {
      setO(2);
    },
  );
  return <div />;
}

export function Shadowed() {
  const createOptimistic = (value: number): [() => number, (next: number) => void] => [() => value, () => {}];
  const [, setO] = createOptimistic(1);
  createEffect(
    () => 1,
    () => {
      setO(2);
    },
  );
  return <div />;
}
