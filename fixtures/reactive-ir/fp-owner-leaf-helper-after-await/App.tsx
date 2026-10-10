import { onCleanup, onSettled } from "solid-js";

// Negative for leaf-owner-forbidden-call: the cleanup runs after the first
// await, from a promise continuation. The leaf scope that called the helper is
// gone and no owner exists at all -- a different claim, `missing-owner`'s.
async function cleanupAfterAwait() {
  await fetch("/api/item");
  onCleanup(() => {});
}

export function AfterAwait() {
  onSettled(() => {
    void cleanupAfterAwait();
  });
  return <div />;
}

// Positive control: the cleanup is registered before the first await, while
// the helper still runs synchronously inside the onSettled callback.
async function cleanupBeforeAwait() {
  onCleanup(() => {});
  await fetch("/api/item");
}

export function BeforeAwait() {
  onSettled(() => {
    void cleanupBeforeAwait();
  });
  return <div />;
}
