import { createEffect, createMemo, untrack } from "solid-js";

// ADR 1020: a function a primitive's callback returns is a value the runtime hands on (an effect's apply receives it, a memo stores
// it). It runs only where something calls that value; reach analysis does not read that, so a site in it is undecided: uncertifiable.

// Violation: the sync memo's compute returns a Promise where the component runs (Chrome: SYNC_NODE_RECEIVED_ASYNC).
export function DirectSync() {
  createMemo(async () => 1, { sync: true });
  return <div />;
}

// Obligation: the effect's compute returns a closure, the apply only reads it: the closure never runs (Chrome is silent).
export function ReturnedToApply() {
  createEffect(
    () => () => {
      createMemo(async () => 2, { sync: true });
    },
    (v) => {
      void v;
    },
  );
  return <div />;
}

// Obligation: the same under `untrack`.
export function ReturnedUnderUntrack() {
  untrack(() => {
    createEffect(
      () => () => {
        createMemo(async () => 3, { sync: true });
      },
      (v) => {
        void v;
      },
    );
  });
  return <div />;
}

// Obligation: a memo's compute returns a closure through a `return`; nothing calls the memo's value.
export function ReturnedFromMemo() {
  createMemo(() => {
    return () => {
      createMemo(async () => 4, { sync: true });
    };
  });
  return <div />;
}

// Violations: the apply calls the value the compute returned (#271's smoke census: MISSING_EFFECT_FN, Chrome reports it), so the
// closure runs and the effect created there without an effect function is reported.
export function AppliedVoidCall() {
  createEffect(
    () => () => {
      createEffect(() => 1, undefined as unknown as () => void);
    },
    (v) => {
      void v();
    },
  );
  return <div />;
}

export function AppliedVoidCallUntrack() {
  untrack(() => {
    createEffect(
      () => () => {
        createEffect(() => 1, undefined as unknown as () => void);
      },
      (v) => {
        void v();
      },
    );
  });
  return <div />;
}
