import { createMemo, createSignal, from } from "solid-js";

// The subscribe function runs in the memo's pass: a read tracks, and no strict
// window holds it (ADR 0622).
export function Reads() {
  const [rate] = createSignal(1);
  const ticks = createMemo(() =>
    from<number>((next) => {
      next(rate());
    }),
  );
  return <p>{ticks()}</p>;
}

// Whether a computation consumes the result is not known at the call, so a
// write in it is unproven either way.
export function Writes() {
  const [count, setCount] = createSignal(0);
  const ticks = createMemo(() =>
    from<number>((next) => {
      setCount(1);
      next(count());
    }),
  );
  return <p>{ticks()}</p>;
}
