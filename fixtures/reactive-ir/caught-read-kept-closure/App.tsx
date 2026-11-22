import { createEffect, createMemo } from "solid-js";

// ADR 1176: a read in a `try` inside a local closure the computation calls in place is the computation's caught read, when the
// closure reads nothing outside a kept block: rc.21 reports CAUGHT_REACTIVE_READ.

// Violation: a memo calls the closure.
export function MemoClosure() {
  const a1 = createMemo(async () => 1);
  const m1 = createMemo(() => {
    const f1 = () => {
      try {
        a1();
      } catch {}
    };
    f1();
    return 1;
  });
  return <p>{m1()}</p>;
}

// Violation: an effect's compute calls the closure.
export function EffectClosure() {
  const a1 = createMemo(async () => 1);
  createEffect(
    () => {
      const f1 = () => {
        try {
          a1();
        } catch {}
      };
      f1();
      return 1;
    },
    () => {},
  );
  return <p />;
}

// Uncertifiable: the closure also reads outside its `try`, so the pass may leave before the caught read.
export function TwinReadOutside() {
  const a1 = createMemo(async () => 1);
  const m1 = createMemo(() => {
    const f1 = () => {
      a1();
      try {
        a1();
      } catch {}
    };
    f1();
    return 1;
  });
  return <p>{m1()}</p>;
}

// Uncertifiable: the closure holds a nested function.
export function TwinNested() {
  const a1 = createMemo(async () => 1);
  const m1 = createMemo(() => {
    const f1 = () => {
      const g = () => 0;
      try {
        a1();
      } catch {}
      return g;
    };
    f1();
    return 1;
  });
  return <p>{m1()}</p>;
}
