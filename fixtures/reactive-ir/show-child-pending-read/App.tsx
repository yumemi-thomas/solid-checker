import { createMemo, Show } from "solid-js";

// ADR 0836: a `Show` child with a parameter runs untracked. A closure handed
// on from it runs where its carrier runs it, so a pending read there is no
// rendered read: rc.21 in Chrome reports PENDING_ASYNC_UNTRACKED_READ only.
// The forward calls it on the child's stack (ADR 0931): SC5001's violation,
// never an SC5003 one.
export function Forwarded() {
  const pending = createMemo(async () => 1);
  const hc0 = (g: () => void) => { g(); };
  const hc1 = (g: () => void) => { hc0(g); };
  return (
    <Show when={1}>
      {(_u) => {
        hc1(() => {
          pending();
        });
        return <p>x</p>;
      }}
    </Show>
  );
}

// Kept: the same read written in the child's body is SC5001's violation.
export function InChild() {
  const pending = createMemo(async () => 1);
  return (
    <Show when={1}>
      {(_u) => {
        pending();
        return <p>x</p>;
      }}
    </Show>
  );
}
