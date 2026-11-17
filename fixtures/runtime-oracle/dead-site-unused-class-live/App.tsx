import { createEffect, createMemo, createSignal } from "solid-js";

// Each component reads `r` after an `await` in an async memo, inside a construct the memo defines.
const later = () => Promise.resolve();
function watch(m: () => number) {
  createEffect(() => m(), () => {});
}

// ---- May run: the construct is triggered, or the value leaves; today's reading stays (a violation) ----

export function CtorNew() {
  const [r] = createSignal(1);
  const m = createMemo(async () => {
    await later();
    class K {
      constructor() {
        r();
      }
    }
    new K();
    return 1;
  });
  watch(m);
  return <p />;
}
export function CtorSuperNew() {
  const [r] = createSignal(1);
  const m = createMemo(async () => {
    await later();
    class K1 {
      constructor() {
        r();
      }
    }
    class K2 extends K1 {}
    new K2();
    return 1;
  });
  watch(m);
  return <p />;
}
export function FieldHelperNew() {
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
    void new K();
    return 1;
  });
  watch(m);
  return <p />;
}
export function GetterRead() {
  const [r] = createSignal(1);
  const m = createMemo(async () => {
    await later();
    const o = {
      get v() {
        r();
        return 1;
      },
    };
    void o.v;
    return 1;
  });
  watch(m);
  return <p />;
}
// The class leaves through a wrapper's parameter, and through `Reflect.construct`.
export function Wrapped() {
  const [r] = createSignal(1);
  const m = createMemo(async () => {
    await later();
    class K {
      constructor() {
        r();
      }
    }
    const make = (C: new () => object) => new C();
    make(K);
    return 1;
  });
  watch(m);
  return <p />;
}
export function Reflected() {
  const [r] = createSignal(1);
  const m = createMemo(async () => {
    await later();
    const o = {
      get v() {
        r();
        return 1;
      },
    };
    Reflect.get(o, "v");
    return 1;
  });
  watch(m);
  return <p />;
}
