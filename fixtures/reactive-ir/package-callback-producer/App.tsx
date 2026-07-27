import { createMemo, onSettled } from "solid-js";

export function runInline(callback: () => void) {
  callback();
}

export function runTracked(callback: () => void) {
  createMemo(() => callback());
}

export function runDeferred(callback: () => void) {
  onSettled(() => callback());
}

export function forwardInline(callback: () => void) {
  runInline(callback);
}

export function Probe() {
  return <div />;
}
