import { createEffect, createMemo, createSignal } from "solid-js";

// ADR 0479: a function bound to a name whose only reference is one call runs
// on that call's stack, so the layer climb crosses it. rc.17 throws
// REACTIVE_WRITE_IN_OWNED_SCOPE for a signal write in a memo or an effect's
// compute, and lets it land in an event handler.

// Violation: the closure's one call is in the memo.
export function OneCallInMemo() {
  const [count, setCount] = createSignal(0);
  const bump = () => setCount(1);
  const m = createMemo(() => {
    bump();
    return 1;
  });
  return <div>{count()}{m()}</div>;
}

// Clean: the closure's one call is in a handler, nested in the component; the
// walk follows the call into the handler, not the component body.
export function OneCallInHandler() {
  const [count, setCount] = createSignal(0);
  const bump = () => setCount(1);
  return <button onClick={() => bump()}>{count()}</button>;
}

// Not proven in place: a named function expression is also entered by its own
// name, and the binding is handed to a timer.
export function NamedExpressionEscapes() {
  const [count, setCount] = createSignal(0);
  const g = function f() {
    setCount(1);
  };
  const m = createMemo(() => {
    g();
    return 1;
  });
  setTimeout(g, 0);
  return <div>{count()}{m()}</div>;
}

// Not proven in place: a second reference passes the closure on.
export function Passed() {
  const [count, setCount] = createSignal(0);
  const bump = () => setCount(1);
  const m = createMemo(() => {
    bump();
    return 1;
  });
  queueMicrotask(bump);
  return <div>{count()}{m()}</div>;
}

// Not proven in place: an exported function may have callers outside the program.
const [shared, setShared] = createSignal(0);
export function exportedWrite() {
  setShared(1);
}
export function ExportedCalledOnce() {
  const m = createMemo(() => {
    exportedWrite();
    return shared();
  });
  return <div>{m()}</div>;
}

// Violation (ADR 0489): a read in a component-level closure, called only from
// a closure written in a createEffect apply and called once there, runs in the
// apply, untracked (SC1001).
export function ReadThroughNestedClosures() {
  const [count] = createSignal(0);
  const peek = () => {
    count();
  };
  createEffect(
    () => 1,
    () => {
      const relay = () => {
        peek();
      };
      relay();
    },
  );
  return <div></div>;
}
