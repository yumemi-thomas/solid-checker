import { createEffect, createMemo, createSignal, onCleanup } from "solid-js";


// ---- The body of a `for await` runs only after the loop's first `next()` was awaited (ADR 1038): it is past a suspension, with no owner ----

// Clean: a write past the suspension is no write in an owned scope.
export function WriteInBody() {
  const [, setW] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      setW(1);
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// An owner-requiring call past the suspension has no owner (Chrome: NO_OWNER_CLEANUP). A violation since ADR 1165: the body over `[1]` runs, each
// run past the loop's await, whoever calls the component (also `mx-no-owner-cleanup-memo-async-for-await-*`).
export function CleanupInBody() {
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      onCleanup(() => {});
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}

// ---- Not past the suspension, or not proven so: today's reading stays ----

// The iterable is evaluated before the loop suspends: the write is in the memo's owned scope (violation).
export function WriteInIterable() {
  const [, setW] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of (setW(2), [1])) {
      void _x;
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// A `finally` around the loop also runs when the iterable throws synchronously, before any suspension (not proven: uncertifiable).
export function WriteInFinally() {
  const [, setW] = createSignal(0);
  const m = createMemo(async () => {
    try {
      for await (const _x of [1]) {
        void _x;
      }
    } finally {
      setW(3);
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// After the loop: not this ADR's region.
export function WriteAfterLoop() {
  const [, setW] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      void _x;
    }
    setW(4);
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
