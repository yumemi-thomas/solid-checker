import { createEffect, createMemo, getOwner, onCleanup } from "solid-js";

// False at run time; nothing proves it.
const flag = Math.random() > 2;

// ---- An owner call in a counted loop's later pass (ADR 1185): `if (i > 0)` runs only after a pass that awaited ----

// Violation: an effect created on the second pass of an async memo's loop (Chrome: NO_OWNER_EFFECT).
export function EffectLaterPass() {
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        createEffect(() => 1, () => {});
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Violation: `i++` and `<= 1` (Chrome: NO_OWNER_CLEANUP).
export function CleanupLaterPass() {
  const m = createMemo(async () => {
    for (let i = 0; i <= 1; i++) {
      if (i > 0) {
        onCleanup(() => {});
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Violation: an async helper the component calls (its first pass is owned and skips the site).
async function helperLoop() {
  for (let i = 0; i < 2; ++i) {
    if (i > 0) {
      onCleanup(() => {});
    }
    await Promise.resolve();
  }
}
export function HelperLaterPass() {
  void helperLoop();
  return <p />;
}
// Violation: through an IIFE run in place (ADR 0493's climb).
export function IifeLaterPass() {
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        (() => {
          onCleanup(() => {});
        })();
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}

// ---- Not proven: uncertifiable as before ----

// No guard: the first pass runs the site owned.
export function CleanupEveryPass() {
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      onCleanup(() => {});
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// A `continue` may skip the await, so a later pass may follow no suspension.
export function CleanupAfterContinue() {
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        onCleanup(() => {});
      }
      if (flag) continue;
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// The await is conditional.
export function CleanupConditionalAwait() {
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        onCleanup(() => {});
      }
      if (flag) {
        await Promise.resolve();
      }
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// The body writes the counter: `i > 0` no longer means a pass completed.
export function CleanupCounterWritten() {
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        onCleanup(() => {});
      }
      if (flag) i = 1;
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// The guard names a shadowing binding, not the counter.
export function CleanupShadowedCounter() {
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      {
        const i = 1;
        if (i > 0) {
          onCleanup(() => {});
        }
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// One pass only: the guarded site never runs (a dead site).
export function CleanupSinglePass() {
  const m = createMemo(async () => {
    for (let i = 0; i < 1; i += 1) {
      if (i > 0) {
        onCleanup(() => {});
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// The function probes `getOwner()`: a guard may skip the site with no owner.
export function CleanupOwnerGuarded() {
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0 && getOwner()) {
        onCleanup(() => {});
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
