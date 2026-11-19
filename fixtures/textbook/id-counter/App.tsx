import { createSignal } from "solid-js";

export function Counter() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <span>{count()}</span>
      <button type="button" onClick={() => setCount((n) => n + 1)}>
        +
      </button>
      <button type="button" onClick={() => setCount(0)}>
        reset
      </button>
    </div>
  );
}
