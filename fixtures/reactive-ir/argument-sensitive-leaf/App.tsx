import { createEffect, onCleanup, onSettled } from "solid-js";

// The `@solid-primitives/timer` shape.
function createTimer(fn: () => void, delay: number | (() => number)) {
  if (typeof delay === "number") {
    onCleanup(() => fn());
    return;
  }
  createEffect(() => delay(), () => fn());
}

declare const either: number | (() => number);

// Violation for `onCleanup` only: a number takes that arm and returns.
export function NumberDelay() {
  onSettled(() => {
    createTimer(() => {}, 30_000);
  });
  return <div />;
}

// Violation for `createEffect` only: a function skips the cleanup arm.
export function AccessorDelay() {
  onSettled(() => {
    createTimer(() => {}, () => 1000);
  });
  return <div />;
}

// Uncertifiable, twice: the argument decides neither arm.
export function EitherDelay() {
  onSettled(() => {
    createTimer(() => {}, either);
  });
  return <div />;
}
