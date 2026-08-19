import { createMemo, onSettled } from "solid-js";

export function runInline(callback: () => void) {
  callback();
}

export function runTracked(callback: () => void) {
  createMemo(() => callback());
}

// The callback is deferred, but it also runs inside onSettled's conditional
// leaf owner. Schema-v1 callback timing cannot certify that owner role, so the
// exported surface remains SC9012 rather than silently publishing only
// "deferred" and losing the leaf restriction.
export function runDeferred(callback: () => void) {
  onSettled(() => callback());
}

export function forwardInline(callback: () => void) {
  runInline(callback);
}

export function Probe() {
  return <div />;
}
