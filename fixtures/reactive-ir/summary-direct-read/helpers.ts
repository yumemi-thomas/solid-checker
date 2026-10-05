import { createSignal } from "solid-js";

export const [count] = createSignal(0);

// Reads directly in its own synchronous body.
export function readNow() {
  return count() + 1;
}

// Reads its accessor argument directly in its own body.
export function readArgument(read: () => number) {
  return read() + 1;
}

// Reads only in a closure it returns.
export function readLater() {
  return () => count();
}

// Reads after an await.
export async function readAfterAwait() {
  await Promise.resolve();
  return count();
}

// Reads in a default-parameter initializer, never in its body.
export function readDefault(value = count()) {
  return value;
}

// Reads only through another helper.
export function readThroughHelper() {
  return readNow();
}
