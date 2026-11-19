import { createMemo, createSignal } from "solid-js";

export function Doubler() {
  const [count, setCount] = createSignal(1);
  const doubled = createMemo(() => count() * 2);
  return (
    <button type="button" onClick={() => setCount((n) => n + 1)}>
      {count()} doubled is {doubled()}
    </button>
  );
}
