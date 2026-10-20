import { createSignal } from "solid-js";

// Past an await that dominates it, a write runs on a later task with no
// owner: legal, and silent in the checker.
export function AfterAwait() {
  const [x, setX] = createSignal(0);
  const load = async () => {
    try {
      const value = await Promise.resolve(1);
      setX(value);
    } catch {
      // unreachable here
    }
  };
  void load();
  return <p>{x()}</p>;
}

// Control: before the await, the write runs in the component body's window.
export function BeforeAwait() {
  const [x, setX] = createSignal(0);
  const load = async () => {
    try {
      setX(2);
      await Promise.resolve();
    } catch {
      // unreachable here
    }
  };
  void load();
  return <p>{x()}</p>;
}
