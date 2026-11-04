import { createOptimistic, createOptimisticStore, createRoot } from "solid-js";

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

// The creating stack, in any role: a closure, a root's callback, a handler
// that creates the source.
export function OnTheStack() {
  const [, setValue] = createOptimistic(async () => 1);
  const write = () => setValue(2);
  write();
  createRoot(() => {
    setValue(3);
  });
  return (
    <button
      onClick={() => {
        const [, setOwn] = createOptimistic(async () => 1);
        setOwn(4);
      }}
    >
      x
    </button>
  );
}

// Off the stack it stays open: the click may land after the answer.
export function Later() {
  const [, setValue] = createOptimistic(async () => 1);
  return <button onClick={() => setValue(2)}>x</button>;
}
