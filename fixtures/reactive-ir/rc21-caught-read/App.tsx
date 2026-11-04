import { createMemo, createSignal } from "solid-js";

// The compute runs where it is created, the memo pending: it catches and completes.
export function Caught() {
  const memo = createMemo(async () => 1);
  const safe = createMemo(() => {
    try {
      return memo();
    } catch {
      return 0;
    }
  });
  return <p>{safe()}</p>;
}

// A rethrow lets the suspension go on.
export function Rethrown() {
  const memo = createMemo(async () => 1);
  const safe = createMemo(() => {
    try {
      return memo();
    } catch (error) {
      throw error;
    }
  });
  return <p>{safe()}</p>;
}

// A memo may throw its error: open. A signal cannot.
export function Open() {
  const [count] = createSignal(1);
  const inner = createMemo(() => count());
  const one = createMemo(() => {
    try {
      return inner();
    } catch {
      return 0;
    }
  });
  const two = createMemo(() => {
    try {
      return count();
    } catch {
      return 0;
    }
  });
  return <p>{[one(), two()].length}</p>;
}
