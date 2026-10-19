import { createEffect, createMemo, createSignal } from "solid-js";

// ---- A write no fact places is an obligation (SC2001 uncertifiable).

export function StoredClosure() {
  const [tab, setTab] = createSignal("a");
  // The memo only stores the closure; who calls `go`, and when, is unseen.
  const actions = createMemo(() => ({ go: () => setTab("b") }));
  return <div>{tab()}{String(actions())}</div>;
}

// An exported helper with no caller in the project.
const [count, setCount] = createSignal(0);
export function bump() {
  setCount(count() + 1);
}

// ---- Controls.

export function BodyWrite() {
  const [x, setX] = createSignal(0);
  // A proven owned-scope write stays a violation.
  setX(1);
  return <div>{x()}</div>;
}

export function PlacedWrites() {
  const [x, setX] = createSignal(0);
  // An intrinsic event handler and an effect apply are placed: no finding.
  createEffect(() => x(), (value) => setX(value + 1));
  return <button onClick={() => setX(2)}>{x()}</button>;
}

// ---- A `for await` under a branch may never run, so the code after it may
// still be in the body's window: unplaced, not proven past it (SC2001 and
// SC1001 uncertifiable).

declare const flag: boolean;
async function ConditionalLoop() {
  const [x, setX] = createSignal(0);
  if (flag) {
    for await (const v of [1]) {
      void v;
    }
  }
  setX(1);
  const value = x();
  return <div>{value}</div>;
}
export const loop = <ConditionalLoop />;
