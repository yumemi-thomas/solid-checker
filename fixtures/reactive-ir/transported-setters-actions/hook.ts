import { action, createOptimistic, createSignal } from "solid-js";

// Each hook returns one fresh object literal whose members are a primitive's
// own setter and action bindings: transport carries them to the caller.
export function useCounter() {
  const [count, setCount] = createSignal(0);
  const save = action(function* () {
    setCount(1);
  });
  return { count, setCount, save };
}

export function useOptimistic() {
  const [count, setCount] = createOptimistic(0);
  return { count, setCount };
}
