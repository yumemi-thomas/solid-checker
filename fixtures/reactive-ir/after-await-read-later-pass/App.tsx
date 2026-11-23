import { createEffect, createMemo, createSignal, untrack } from "solid-js";

// False at run time; nothing proves it.
const flag = Math.random() > 2;

function runNow(cb: () => void) {
  cb();
}

// ---- A read placed past the await of an async computation (ADR 1191): a counted loop's later pass, and layers run in place ----

// Violation: the second pass reads after the first pass's await, in the memo's flight.
export function LaterPassRead() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        s();
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Violation: an IIFE in the later pass.
export function LaterPassIife() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i++) {
      if (i > 0) {
        (() => {
          s();
        })();
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Violation: a closure bound to a name and called once in a `for await` body that runs.
export function ForAwaitClosure() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      const f = () => {
        s();
      };
      f();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Violation: a project helper's callback, run in place in a `for await` body (an effect's compute).
export function ForAwaitHelper() {
  const [s] = createSignal(0);
  createEffect(async () => {
    for await (const _x of [1]) {
      runNow(() => {
        s();
      });
    }
    return 1;
  }, () => {});
  return <p />;
}

// ---- Not proven: uncertifiable as before ----

// The first pass reads before any await: no guard.
export function EveryPassRead() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      s();
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// A `continue` may skip the await.
export function LaterPassAfterContinue() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        s();
      }
      if (flag) continue;
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// The guard names a shadowing binding.
export function LaterPassShadowed() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      {
        const i = 1;
        if (i > 0) {
          s();
        }
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// A library callback, which may run zero times: not a layer run in place.
export function ForAwaitLibraryCallback() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      [1].forEach(() => {
        s();
      });
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// `untrack`'s callback: a primitive's slot, not a project layer.
export function ForAwaitUntrack() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      untrack(() => {
        s();
      });
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// A closure the body hands on rather than calls: it may run anywhere.
export function ForAwaitClosureKept() {
  const [s] = createSignal(0);
  let kept: (() => void) | undefined;
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      kept = () => {
        s();
      };
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  void kept;
  return <p />;
}
