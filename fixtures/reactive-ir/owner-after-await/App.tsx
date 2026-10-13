import { createEffect, createSignal, getOwner, onCleanup, runWithOwner } from "solid-js";

// Positive: the helper is entered under the component's owner, but the cleanup
// is registered after the await, from a promise continuation, where no owner
// is current. It never runs.
async function cleanupAfterAwait() {
  await fetch("/api/a");
  onCleanup(() => {});
}

export function CleanupAfterAwait() {
  void cleanupAfterAwait();
  return <div />;
}

// Positive: an effect created after the await is never disposed.
async function effectAfterAwait(count: () => number) {
  const response = await fetch("/api/b");
  createEffect(
    () => count(),
    (value) => {
      void response;
      void value;
    },
  );
}

export function EffectAfterAwait() {
  const [count] = createSignal(0);
  void effectAfterAwait(count);
  return <div />;
}

// Positive: inside the try block the cleanup is reached only after the await.
async function cleanupInTry() {
  try {
    await fetch("/api/c");
    onCleanup(() => {});
  } catch {
    // ignored
  }
}

export function CleanupInTry() {
  void cleanupInTry();
  return <div />;
}

// Positive: an exported helper with no caller in the project. Whoever calls
// it, the continuation has no owner.
export async function exportedCleanupAfterAwait() {
  await fetch("/api/d");
  onCleanup(() => {});
}

// Negative: registered before the first await, still under the caller's owner.
async function cleanupBeforeAwait() {
  onCleanup(() => {});
  await fetch("/api/e");
}

export function CleanupBeforeAwait() {
  void cleanupBeforeAwait();
  return <div />;
}

// Negative: the await is on one branch only, so the cleanup can run
// synchronously under the caller's owner.
async function cleanupAfterConditionalAwait(wait: boolean) {
  if (wait) await fetch("/api/f");
  onCleanup(() => {});
}

export function ConditionalAwait() {
  void cleanupAfterConditionalAwait(false);
  return <div />;
}

// Negative: the catch path reaches the cleanup without awaiting.
async function cleanupAfterTryCatch() {
  try {
    await fetch("/api/g");
  } catch {
    // ignored
  }
  onCleanup(() => {});
}

export function AfterTryCatch() {
  void cleanupAfterTryCatch();
  return <div />;
}

// Not a violation: the owner captured before the await is restored for the
// cleanup. The callback is a nested closure, so it is not an after-await call.
async function cleanupUnderCapturedOwner() {
  const owner = getOwner();
  await fetch("/api/h");
  runWithOwner(owner, () => onCleanup(() => {}));
}

export function CapturedOwner() {
  void cleanupUnderCapturedOwner();
  return <div />;
}
