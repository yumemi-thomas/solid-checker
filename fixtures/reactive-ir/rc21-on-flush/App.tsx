import { createEffect, createMemo, createSignal, onFlush } from "solid-js";

// The callback runs on a microtask with no owner: a write is legal, and an
// effect it creates has no owner (ADR 0622).
export function Handler() {
  const [open, setOpen] = createSignal(false);
  const click = () => {
    onFlush(() => {
      setOpen(true);
    });
  };
  return <button onClick={click}>{String(open())}</button>;
}

export function InApply() {
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    onFlush(() => {
      setCount(1);
    });
  });
  return <p>{count()}</p>;
}

export function NoOwner() {
  const click = () => {
    onFlush(() => {
      createEffect(
        () => 0,
        () => {},
      );
    });
  };
  return <button onClick={click}>x</button>;
}

export function ReadsNoWindow() {
  const [count] = createSignal(0);
  const memo = createMemo(() => 1);
  onFlush(() => {
    console.log(count(), memo());
  });
  return <p>{memo()}</p>;
}
