import { createSignal } from "solid-js";

const [w] = createSignal(0);

// The read runs on a microtask, outside any tracking scope: no violation. It keeps an obligation of its own (`callee-callback-timing`).
export function LateRead() {
  const t = { forEach(f: () => void) { queueMicrotask(f); } };
  (t as unknown as number[]).forEach(() => { w(); });
  return <p />;
}
