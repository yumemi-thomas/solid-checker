import {
  action, createEffect, createMemo, createRoot, createSignal, createTrackedEffect, flush, getOwner, isPending, latest,
  onCleanup, onSettled, resolve, runWithOwner, untrack,
} from "solid-js";

// Positives. An action's step runs with `actionDepth > 0`; a callback that runs in place runs inside it, so `flush()`
// throws FLUSH_IN_ACTION there (`@solidjs/signals@2.0.0-rc.21` `dist/dev-shared.js:3033`).
export function UntrackFlush() {
  const run = action(function* () {
    untrack(() => {
      flush();
    });
  });
  return <button onClick={() => run()}>go</button>;
}

export function LatestFlush() {
  const run = action(function* () {
    latest(() => {
      flush();
    });
  });
  return <button onClick={() => run()}>go</button>;
}

export function RootFlush() {
  const run = action(function* () {
    createRoot(() => {
      flush();
    });
  });
  return <button onClick={() => run()}>go</button>;
}

export function OwnerFlush() {
  const run = action(function* () {
    const owner = getOwner();
    runWithOwner(owner, () => {
      flush();
    });
  });
  return <button onClick={() => run()}>go</button>;
}

function flushes() {
  flush();
}

export function HelperInUntrack() {
  const run = action(function* () {
    untrack(() => flushes());
  });
  return <button onClick={() => run()}>go</button>;
}

// `flush(fn)` throws before it runs `fn`: one finding, at the outer call.
export function FlushInFlush() {
  const run = action(function* () {
    flush(() => {
      flush();
    });
  });
  return <button onClick={() => run()}>go</button>;
}

// Negative. `isPending` catches the throw (no diagnostic precedes it): nothing is reported.
export function CaughtThrow() {
  const run = action(function* () {
    isPending(() => {
      flush();
    });
  });
  return <button onClick={() => run()}>go</button>;
}

// Negative. A handler's callback runs later, outside any step.
export function Deferred() {
  const run = action(function* () {
    setTimeout(() => {
      untrack(() => {
        flush();
      });
    });
  });
  return <button onClick={() => run()}>go</button>;
}
