import { createEffect, createMemo, createSignal } from "solid-js";

// Each component reads `r` after an `await` in an async memo, inside a construct the memo defines.
const later = () => Promise.resolve();
function watch(m: () => number) {
  createEffect(() => m(), () => {});
}

// ---- Never run: nothing constructs the class or accesses the object; the read is U (unreached), never a violation ----

export function CtorNever() {
  const [r] = createSignal(1);
  const m = createMemo(async () => {
    await later();
    class K {
      constructor() {
        r();
      }
    }
    void K;
    return 1;
  });
  watch(m);
  return <p />;
}
// The superclass is reached only through the `extends` of a class nothing reaches.
export function CtorSuperNever() {
  const [r] = createSignal(1);
  const m = createMemo(async () => {
    await later();
    class K1 {
      constructor() {
        r();
      }
    }
    class K2 extends K1 {}
    typeof K2;
    return 1;
  });
  watch(m);
  return <p />;
}
// An instance field's initializer runs only when an instance is made.
export function FieldHelperNever() {
  const [r] = createSignal(1);
  const m = createMemo(async () => {
    await later();
    const fh = () => {
      r();
      return 1;
    };
    class K {
      f = fh();
    }
    K;
    return 1;
  });
  watch(m);
  return <p />;
}
export function GetterNever() {
  const [r] = createSignal(1);
  const m = createMemo(async () => {
    await later();
    const o = {
      get v() {
        r();
        return 1;
      },
    };
    void o;
    return 1;
  });
  watch(m);
  return <p />;
}
export function MethodNever() {
  const [r] = createSignal(1);
  const m = createMemo(async () => {
    await later();
    const o = {
      run() {
        r();
      },
    };
    void (o as unknown);
    return 1;
  });
  watch(m);
  return <p />;
}
