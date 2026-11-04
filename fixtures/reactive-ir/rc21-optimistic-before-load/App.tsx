import { createOptimistic, createOptimisticStore } from "solid-js";

// The setter throws OPTIMISTIC_WRITE_BEFORE_LOAD here on rc.21: an obligation.
export function BeforeLoad() {
  const [value, setValue] = createOptimistic(async () => 1);
  const [state, setState] = createOptimisticStore(async () => ({ a: 1 }), { a: 0 });
  setValue(2);
  setState((draft) => {
    draft.a = 2;
  });
  return <p>{value()}{state.a}</p>;
}

// A sync source has its answer: an unguarded signal setter, and SC2001 for the store.
export function Loaded() {
  const [value, setValue] = createOptimistic(1);
  const [state, setState] = createOptimisticStore({ a: 0 });
  setValue(2);
  setState((draft) => {
    draft.a = 2;
  });
  return <p>{value()}{state.a}</p>;
}
