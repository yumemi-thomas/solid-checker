import { createEffect, createMemo, createRoot, createSignal, flush } from "solid-js";

// ADR 1220: ADR 1205 took a creation in a function an effect's apply calls as creation-throws-first (the effect-callback
// guard throws PRIMITIVE_IN_EFFECT_CALLBACK before SC7002's result probe). For SC7002 that is the apply's first run, and an
// apply written with options may have none (`defer: true`): its runs are reruns a `flush()` may start anywhere, and there
// Chrome reaches the probe (SYNC_NODE_RECEIVED_ASYNC).

// Violation: the deferred apply's closure.
export function DeferredApplyClosure() {
  const [t1, setT1] = createSignal(1);
  createEffect(
    () => t1(),
    () => {
      const f1 = () => {
        createMemo(async () => 1, { sync: true });
      };
      f1();
    },
    { defer: true },
  );
  return (
    <button
      onClick={() => {
        setT1(2);
        createRoot(() => {
          flush();
        });
      }}
    >
      go
    </button>
  );
}

// Uncertifiable, creation-throws-first: the apply's first run creates in its closure, and the guard throws first.
export function ApplyClosure() {
  const [t1] = createSignal(1);
  createEffect(
    () => t1(),
    () => {
      const f1 = () => {
        createMemo(async () => 1, { sync: true });
      };
      f1();
    },
  );
  return <div />;
}
