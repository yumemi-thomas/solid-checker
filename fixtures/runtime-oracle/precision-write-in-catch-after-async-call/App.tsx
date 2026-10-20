import { createSignal } from "solid-js";

async function rejects(): Promise<number> {
  throw new Error("rejected");
}
function throwsNow(): Promise<number> {
  throw new Error("thrown synchronously");
}

// The awaited async call rejects; the catch runs after the await suspended,
// with no owner: legal, and silent in the checker.
export function CatchAfterAsyncCall() {
  const [x, setX] = createSignal(0);
  const run = async () => {
    try {
      await rejects();
    } catch {
      setX(1);
    }
  };
  void run();
  return <p>{x()}</p>;
}

// Control: a plain function throws before any suspension, so the catch runs
// in the body's window (the checker keeps an obligation here).
export function CatchAfterSyncThrow() {
  const [x, setX] = createSignal(0);
  const run = async () => {
    try {
      await throwsNow();
    } catch {
      setX(2);
    }
  };
  void run();
  return <p>{x()}</p>;
}
