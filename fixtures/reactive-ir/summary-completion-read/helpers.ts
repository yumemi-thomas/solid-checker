import { createSignal, createStore } from "solid-js";

export const [count] = createSignal(0);
export const [state] = createStore({ total: 0 });

// Reads on every completion.
export function readAlways() {
  return count() + 1;
}

// The `MaybeAccessor` shape: a number argument returns before the read.
export function readWhen(value: number | (() => number)) {
  if (typeof value === "number") return value;
  return count();
}

// The read sits behind the helper's own branch.
export function readGuarded(flag: boolean) {
  return flag ? count() : 0;
}

// A store path behind a branch.
export function readStoreGuarded(flag: boolean) {
  if (flag) return state.total;
  return 0;
}

// A thrown guard skips the read, and a caller may catch it and carry on.
export function readAfterThrowGuard(ready: boolean) {
  if (!ready) throw new Error("not ready");
  return count();
}

// Every hop unconditional.
export function readThroughAlways() {
  return readAlways();
}

// The second hop is conditional in the first.
export function readThroughWhen(flag: boolean) {
  if (flag) return readAlways();
  return 0;
}

// A default that reads only on one of its own paths.
export function readDefaultGuarded(flag: boolean, value = flag ? count() : 0) {
  return value;
}

// Reads on every completion after a loop that only breaks out of itself:
// a `break` is read as a possible exit, which only proves less.
export function readAfterLoop(items: number[]) {
  for (const item of items) {
    if (item > 1) break;
  }
  return count();
}

// A logical assignment reads its right side only when the target allows.
export function readWhenUnset(target: { value?: number }) {
  target.value ??= count();
  return target.value;
}

export function fail(message: string): never {
  throw new Error(message);
}

// A `never`-returning guard skips the read, and a caller may catch it.
export function readAfterFail(ready: boolean) {
  if (!ready) fail("not ready");
  return count();
}

export function assertReady(ready: boolean): asserts ready {
  if (!ready) throw new Error("not ready");
}

// An `asserts` guard may throw instead of returning (ADR 0279).
export function readAfterAssert(ready: boolean) {
  assertReady(ready);
  return count();
}

// A `never` call in a default runs before the body (ADR 0279).
export function readAfterFailingDefault(ready: boolean, _checked = ready ? 0 : fail("x")) {
  return count();
}
