import { createMemo, createRoot, createTrackedEffect, onCleanup, onSettled } from "solid-js";

// ADR 0492: a primitive written in a callback a project helper calls in its
// own body runs in the leaf owner's callback when the helper call sits there.

function run(cb: () => unknown) {
  cb();
  return 1;
}

function runTwice(cb: () => void) {
  cb();
  cb();
}

function later(cb: () => void) {
  setTimeout(cb, 0);
}

let kept: (() => void) | undefined;
function keep(cb: () => void) {
  kept = cb;
}

async function afterWait(cb: () => void) {
  await Promise.resolve();
  cb();
}

// Violation: a memo created in a helper's callback inside createTrackedEffect.
export function TrackedHelper() {
  createTrackedEffect(() => {
    run(() => {
      createMemo(() => 1);
    });
  });
  return <div />;
}

// Violation: a root created through two helpers inside onSettled.
export function SettledTwoHelpers() {
  onSettled(() => {
    run(() => {
      runTwice(() => {
        createRoot(() => 1);
      });
    });
  });
  return <div />;
}

// Violation: onCleanup in a helper's callback inside createTrackedEffect.
export function CleanupInHelper() {
  createTrackedEffect(() => {
    run(() => {
      onCleanup(() => {});
    });
  });
  return <div />;
}

// Negative: the helper defers its callback to a timer.
export function Deferred() {
  createTrackedEffect(() => {
    later(() => {
      createMemo(() => 1);
    });
  });
  return <div />;
}

// Negative: the helper stores its callback.
export function Stored() {
  onSettled(() => {
    keep(() => {
      createMemo(() => 1);
    });
  });
  return <div />;
}

// Negative: the helper runs its callback after an await.
export function AfterAwait() {
  onSettled(() => {
    void afterWait(() => {
      createMemo(() => 1);
    });
  });
  return <div />;
}

// Negative: the helper call is in an event handler built in the callback.
export function InHandler() {
  onSettled(() => {
    document.body.addEventListener("click", () => {
      run(() => {
        createMemo(() => 1);
      });
    });
  });
  return <div />;
}
