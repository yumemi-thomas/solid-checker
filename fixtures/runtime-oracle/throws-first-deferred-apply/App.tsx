import { createEffect, createMemo, createRoot, createSignal, flush } from "solid-js";

// ADR 1220 (ADR 1205 round 3): ADR 1205 took a creation in a function an effect's apply calls as creation-throws-first for
// SC7002 too. An apply a `flush()` runs (during render, or from a handler when it has no first run: deferred, a reaction)
// meets no effect-callback guard first, and Chrome reaches the probe (SYNC_NODE_RECEIVED_ASYNC). SC7002 keeps main's answer.

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

// Violation: a `flush()` in the body runs the apply's first run itself, before the scheduler's guarded run.
export function FlushBodyClosure() {
  createEffect(
    () => 1,
    () => {
      const f1 = () => {
        createMemo(async () => 1, { sync: true });
      };
      f1();
    },
  );
  flush();
  return <div />;
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
