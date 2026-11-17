import { action, createMemo, createSignal } from "solid-js";

const go = action(function* () {});

// Violations (ADR 1022): a construct-only class's members run at its one `new`
// or receiver call, a statement of the component body (Chrome, rc.21: each
// reports as the same code written in the body does).
export function MethodRead() {
  const m1 = createMemo(() => 1);
  class K {
    m() {
      m1();
    }
  }
  new K().m();
  return <i />;
}

export function CtorWrite() {
  const [, setW] = createSignal(0);
  class K {
    constructor() {
      setW(1);
    }
  }
  new K();
  return <i />;
}

export function FieldAction() {
  class K {
    x = go();
  }
  new K();
  return <i />;
}

// A `const` bound class expression, and a receiver under a cast (wrapper).
export function ConstBound() {
  const [, setW] = createSignal(0);
  const K = class {
    m() {
      setW(1);
    }
  };
  (new K() as { m(): void }).m();
  return <i />;
}

// Clean: the class and its `new` are written in a handler, whose code writes legally.
export function InHandler() {
  const [, setW] = createSignal(0);
  return (
    <button
      onClick={() => {
        class K {
          constructor() {
            setW(1);
          }
        }
        new K();
      }}
    />
  );
}

// Obligations, as before: nothing places these members.
// A method no receiver call runs (the 2026-11-17 ruling: never run is no violation).
export function NeverCalled() {
  const [, setW] = createSignal(0);
  class K {
    m() {
      setW(1);
    }
  }
  new K();
  return <i />;
}

// Two constructions.
export function TwoNews() {
  const [, setW] = createSignal(0);
  class K {
    constructor() {
      setW(1);
    }
  }
  new K();
  new K();
  return <i />;
}

// `this` may carry the instance anywhere.
export function UsesThis() {
  const [, setW] = createSignal(0);
  class K {
    n = 0;
    m() {
      this.n = setW(1);
    }
  }
  new K().m();
  return <i />;
}

// The run is in a handler, not where the class is written.
export function RunInHandler() {
  const [, setW] = createSignal(0);
  class K {
    m() {
      setW(1);
    }
  }
  return <button onClick={() => new K().m()} />;
}

// A computed member call.
export function ComputedMember() {
  const [, setW] = createSignal(0);
  class K {
    m() {
      setW(1);
    }
  }
  new K()["m"]();
  return <i />;
}

// Shadowed: the `K` constructed is the parameter, not the class.
export function Shadowed() {
  const [, setW] = createSignal(0);
  class K {
    m() {
      setW(1);
    }
  }
  const make = (K: new () => { m(): void }) => new K().m();
  make(class {
    m() {}
  });
  return <i />;
}

// A closure in the method: its call runs in the body (a violation there); the read written in the closure keeps its obligation.
export function ClosureInMethod() {
  const m1 = createMemo(() => 1);
  class K {
    m() {
      const f = () => m1();
      f();
    }
  }
  new K().m();
  return <i />;
}
