import { createSignal } from "solid-js";

export const [count] = createSignal(0);

// The `MaybeAccessor` shape.
export function readWhen(value: number | (() => number)) {
  if (typeof value === "number") return value;
  return count();
}

export function readGuarded(flag: boolean) {
  return flag ? count() : 0;
}

export function readFallback(value?: number) {
  return value ?? count();
}

export function fail(message: string): never {
  throw new Error(message);
}

export function readAfterGuard(ready: boolean) {
  if (!ready) fail("not ready");
  return count();
}

// A written parameter decides nothing.
export function readWritten(flag: boolean) {
  flag = !flag;
  return flag ? count() : 0;
}

export function runWhen(flag: boolean, cb: () => number) {
  if (flag) return cb();
  return 0;
}
