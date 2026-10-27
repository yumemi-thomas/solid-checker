// app-game abr-viewer src/lib/workspace.ts:49 (App.tsx:100): a factory returns a signal accessor, a memo and a function in
// an object; another component calls them as members of the result, and the accessor and the memo (bindings of calls) are not shown
// free of `this`.
import { createMemo, createSignal } from "solid-js";

export function createCounter() {
  const [count, setCount] = createSignal(0);
  const doubled = createMemo(() => count() * 2);
  return { count, doubled, bump: () => setCount(count() + 1) };
}

export function Counter() {
  const counter = createCounter();
  return (
    <button onClick={() => counter.bump()}>
      {counter.count()} {counter.doubled()}
    </button>
  );
}
