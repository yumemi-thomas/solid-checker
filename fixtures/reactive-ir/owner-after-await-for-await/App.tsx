import { createEffect, createMemo, getOwner, onCleanup, onSettled } from "solid-js";

// ---- An owner call in the body of a `for await` that provably runs it (ADR 1165): every run follows the loop's await, with no owner ----

// Violation: the effect is created past the suspension, in an async memo's compute (Chrome: NO_OWNER_EFFECT).
export function EffectInBody() {
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      createEffect(() => 1, () => {});
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Violation: a cleanup registered past the suspension (Chrome: NO_OWNER_CLEANUP).
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
// Violation: `onSettled` past the suspension, its callback returning a cleanup (Chrome: SETTLED_CLEANUP_UNOWNED).
export function SettledInBody() {
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      onSettled(() => () => {});
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Violation: an async helper the component calls (its first run is owned up to the loop's await).
async function helperLoop() {
  for await (const _x of [1]) {
    onCleanup(() => {});
  }
}
export function HelperInBody() {
  void helperLoop();
  return <p />;
}
// Violation (ADR 1164's try head): a `finally` after `try { await Promise.resolve(); }`.
export function CleanupInFinally() {
  const m = createMemo(async () => {
    try { await Promise.resolve(); } finally {
      onCleanup(() => {});
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}

// ---- Not proven: uncertifiable as before ----

// The body never runs (a dead site, ADR 1018).
export function CleanupInEmptyLoop() {
  const m = createMemo(async () => {
    for await (const _x of []) {
      onCleanup(() => {});
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// A named iterable: whether the body runs is not read.
export function CleanupInNamedLoop() {
  const xs = [1];
  const m = createMemo(async () => {
    for await (const _x of xs) {
      onCleanup(() => {});
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// The function probes `getOwner()`: a guard may skip the loop with no owner (as `call_owner_edge`).
export function CleanupGuarded() {
  const m = createMemo(async () => {
    if (getOwner()) {
      for await (const _x of [1]) {
        onCleanup(() => {});
      }
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// A `finally` around the loop: the iterable may throw before any suspension.
export function CleanupInFinallyAroundLoop() {
  const m = createMemo(async () => {
    try {
      for await (const _x of [1]) {
        void _x;
      }
    } finally {
      onCleanup(() => {});
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
