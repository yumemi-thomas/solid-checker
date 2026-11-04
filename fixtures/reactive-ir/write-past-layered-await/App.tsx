import { createEffect, createMemo, createRoot, createSignal } from "solid-js";

function run(cb: () => void) {
  cb();
}

// Clean: an IIFE, a helper calling its callback and a named closure's one call, each after the compute's await, write on the
// later task with no owner.
export function Layered() {
  const [w, setW] = createSignal(0);
  const m = createMemo(async () => {
    await Promise.resolve();
    (() => {
      setW(1);
    })();
    run(() => {
      setW(2);
    });
    const later = () => {
      setW(3);
    };
    later();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div>{w()}</div>;
}

declare function ready(): boolean;

// Uncertifiable: the await is in a branch, so the IIFE may run on the compute's first stack, under its owner.
export function Branch() {
  const [w, setW] = createSignal(0);
  const m = createMemo(async () => {
    if (ready()) await Promise.resolve();
    (() => {
      setW(4);
    })();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div>{w()}</div>;
}

// Violations (controls): a root created past the await owns its body (rc.17 throws), and an IIFE before the await runs in
// the compute.
export function Controls() {
  const [w, setW] = createSignal(0);
  const m = createMemo(async () => {
    (() => {
      setW(5);
    })();
    await Promise.resolve();
    createRoot(() => {
      setW(6);
    });
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div>{w()}</div>;
}

// A compute passed by name: clean past a dominating await; uncertifiable in a `finally` whose `try` may throw before it
// suspends (the named tracked position is no proof there).
export function Named() {
  const [w, setW] = createSignal(0);
  const after = async () => {
    await Promise.resolve();
    (() => {
      setW(7);
    })();
    return 1;
  };
  const inFinally = async () => {
    try {
      await Promise.resolve();
    } finally {
      const f = () => {
        setW(8);
      };
      f();
    }
    return 1;
  };
  const m = createMemo(after);
  const n = createMemo(inFinally);
  createEffect(() => m() + n(), () => {});
  return <div>{w()}</div>;
}
