import { createEffect, createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
const later = () => new Promise<void>((resolve) => setTimeout(resolve, 5));

// ADR 1105: a class value a helper returns, stored and constructed later. Its instance field initializers and its constructor run
// where `new` runs (here a `toString` that an async memo's template literal runs after its own await), not in the helper that wrote
// the class. Chrome (rc.21) warns UNTRACKED_READ_AFTER_AWAIT at each helper's read below; main 28fde14bf certified `Made`, `MadeCtor`
// and `MadeMethod` clean. Each read is an uncertifiable row of SC1002 (`function-run-unplaced`), never a violation: no exact fact
// says where the stored class is constructed.

// Uncertifiable (probe k1n): an instance field initializer of a class expression `makeField` returns. The walk took the initializer's call
// for code of `makeField` and placed it at `makeField()`, at module scope.
const peekField = () => count();
const fieldBox: { K?: new () => unknown } = {};
const makeField = () =>
  class {
    x = peekField();
  };
fieldBox.K = makeField();
const fieldCoerced = {
  toString() {
    new fieldBox.K!();
    return "";
  },
};
export function Made() {
  const m = createMemo(async () => {
    await later();
    void `${fieldCoerced}`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Uncertifiable: the read is the constructor's. The call graph withholds a call in a constructor (it may run before the helper's
// declarator, ADR 0339), so the member walk found no caller of `peekCtor`.
const peekCtor = () => count();
const ctorBox: { K?: new () => unknown } = {};
const makeCtor = () =>
  class {
    x: number;
    constructor() {
      this.x = peekCtor();
    }
  };
ctorBox.K = makeCtor();
const ctorCoerced = {
  toString() {
    new ctorBox.K!();
    return "";
  },
};
export function MadeCtor() {
  const m = createMemo(async () => {
    await later();
    void `${ctorCoerced}`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Uncertifiable: the constructor calls a method, which reads.
const peekMethod = () => count();
const methodBox: { K?: new () => unknown } = {};
const makeMethod = () =>
  class {
    x: number;
    constructor() {
      this.x = this.get();
    }
    get() {
      return peekMethod();
    }
  };
methodBox.K = makeMethod();
const methodCoerced = {
  toString() {
    new methodBox.K!();
    return "";
  },
};
export function MadeMethod() {
  const m = createMemo(async () => {
    await later();
    void `${methodCoerced}`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Uncertifiable, as on main: a class declaration that escapes (stored), its field read through a helper.
const peekEscaped = () => count();
const escapedBox: { K?: new () => unknown } = {};
class Escaped {
  x = peekEscaped();
}
escapedBox.K = Escaped;
const escapedCoerced = {
  toString() {
    new escapedBox.K!();
    return "";
  },
};
export function Stored() {
  const m = createMemo(async () => {
    await later();
    void `${escapedCoerced}`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// The twin, rows as on main: a class written in place and constructed directly after the await. The continuation runs the
// initializer, so the read is the memo's (SC1002 violation), beside the uncertifiable rows main gives the site.
const peekInPlace = () => count();
export function InPlace() {
  const m = createMemo(async () => {
    await later();
    new (class {
      x = peekInPlace();
    })();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Uncertifiable (the cost; main: clean): a made class nothing constructs. Where a stored class value is constructed is not
// modelled, so no fact proves the initializer never runs.
const peekNever = () => count();
const neverBox: { K?: new () => unknown } = {};
const makeNever = () =>
  class {
    x = peekNever();
  };
neverBox.K = makeNever();

// Clean, as on main: a class declaration nothing names (ADR 1021): no instance is made, so its initializer never runs.
const peekUnused = () => count();
class Unused {
  x = peekUnused();
}
