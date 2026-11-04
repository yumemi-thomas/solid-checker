import { createEffect, createSignal, flush } from "solid-js";

// A parameter default runs at its function's call, here in a component body, not at module evaluation:
// the flush drains under the component's owner, so the apply's write throws.
const drain = (_: unknown = flush()) => {};

export function Default() {
  const [n, setN] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      setN(1);
    },
  );
  drain();
  return <div>{n()}</div>;
}
