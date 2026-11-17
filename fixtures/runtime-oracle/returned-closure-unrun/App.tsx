import { createEffect, createMemo, createSignal, flush, untrack } from "solid-js";

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

// Obligation: the apply calls the returned closure, so the memo is created inside the apply with no owner set: rc.21 throws
// PRIMITIVE_IN_EFFECT_CALLBACK at the creation (SC6002) and never probes the computation, so SC7002 is not proven (Chrome reports
// PRIMITIVE_IN_EFFECT_CALLBACK there, not SYNC_NODE_RECEIVED_ASYNC).
export function ReturnedAndCalled() {
  createEffect(
    () => () => {
      createMemo(async () => 5, { sync: true });
    },
    (v) => {
      v();
    },
  );
  return <div />;
}

// Obligations (third cut): the creation runs only where a returned value is called. A memo's value read by an effect's compute
// and called in the apply, and a helper the returned closure calls: Chrome reports PRIMITIVE_IN_EFFECT_CALLBACK, never
// SYNC_NODE_RECEIVED_ASYNC. The guard is the site's reach, not the closure's text, so neither is a violation.
export function MemoValueCalledInApply() {
  const m = createMemo(() => () => {
    createMemo(async () => 6, { sync: true });
  });
  createEffect(
    () => m(),
    (f) => {
      f();
    },
  );
  return <div />;
}

function makeSync() {
  createMemo(async () => 7, { sync: true });
}

export function HelperCalledInApply() {
  createEffect(
    () => () => {
      makeSync();
    },
    (v) => {
      v();
    },
  );
  return <div />;
}

// Obligation: called in the component body, the sync memo does throw SYNC_NODE_RECEIVED_ASYNC (Chrome), but whether a call of a
// returned value is on an apply's stack is not decided per call: uncertifiable, as on main.
export function MemoValueCalledInBody() {
  const m = createMemo(() => () => {
    createMemo(async () => 8, { sync: true });
  });
  untrack(m)();
  return <div />;
}

// Obligations (fourth cut, Fable's review): a site in a callback of a computation created inside the returned value. Called on the
// apply's stack, that creation throws PRIMITIVE_IN_EFFECT_CALLBACK first and the callback never runs (Chrome: only that code), so no
// rule's claim there is proven: the memo's compute creating an effect with no effect function (SC7001), a flush in a created
// effect's apply (SC2008).
export function CreatedInReturnedMemo() {
  createEffect(
    () => () => {
      createMemo(() => {
        createEffect(() => 1, undefined as unknown as () => void);
        return 1;
      });
    },
    (v) => {
      v();
    },
  );
  return <div />;
}

export function CreatedInReturnedEffect() {
  const [s, setS] = createSignal(0);
  createEffect(
    () => () => {
      createEffect(
        () => s(),
        () => {
          setS(2);
          flush();
        },
      );
    },
    (v) => {
      v();
    },
  );
  return <div />;
}
