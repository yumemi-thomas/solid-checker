import { createEffect, createMemo, createRoot, createTrackedEffect, onFlush, onSettled } from "solid-js";

const done = () => {};

// Under a children-capable owner: rc.21 warns ON_FLUSH_IN_OWNED_SCOPE (ADR 0625).
export function Owned() {
  onFlush(done);
  const memo = createMemo(() => {
    onFlush(done);
    return 1;
  });
  createEffect(
    () => {
      onFlush(done);
      return 0;
    },
    () => {},
  );
  createRoot(() => {
    onFlush(done);
  });
  return <p>{memo()}</p>;
}

// No such owner: silent on rc.21.
export function Meant() {
  createEffect(
    () => 0,
    () => {
      onFlush(done);
    },
  );
  onSettled(() => {
    onFlush(done);
  });
  createTrackedEffect(() => {
    onFlush(done);
  });
  const click = () => {
    onFlush(done);
  };
  return <button onClick={click}>x</button>;
}

// A local name is not the primitive.
export function Shadowed() {
  const onFlush = (fn: () => void) => fn();
  onFlush(done);
  return <p>x</p>;
}
