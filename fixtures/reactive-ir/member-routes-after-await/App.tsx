import { createEffect, createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
const pause = () => Promise.resolve();

// ADR 0963 (batch 10b, group A): the continuation of an async computation runs a member without a call: a property access runs a
// getter, an assignment a setter, a coercion `toString`, a `for...of` or a spread `[Symbol.iterator]`, a Proxy access its trap,
// `new` a constructor and the class's field initializers, a tagged template its tag. The walk of the continuation follows calls
// and runs none of these, so a source read there warns UNTRACKED_READ_AFTER_AWAIT (Chrome, rc.21) while the program was certified
// clean beside SC1001's uncertifiable row. Each read below is an uncertifiable row of SC1002 (`function-run-unplaced`, or
// `field-initializer-unplaced` for a read directly in a field's initializer), never a violation and never silence.

const accessors = {
  get value() {
    return count();
  },
  set value(_next: number) {
    count();
  },
};
export function ObjectAccessor() {
  const m = createMemo(async () => {
    await pause();
    void accessors.value;
    accessors.value = 1;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

class Counter {
  get now() {
    return count();
  }
  total() {
    return count();
  }
}
export function ClassMembers() {
  const counter = new Counter();
  const m = createMemo(async () => {
    await pause();
    void counter.now;
    counter.total();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

const coerced = {
  toString() {
    return String(count());
  },
};
export function Coercion() {
  const m = createMemo(async () => {
    await pause();
    void `${coerced}`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

const iterable = {
  [Symbol.iterator]() {
    count();
    return [1][Symbol.iterator]();
  },
};
export function Iteration() {
  const m = createMemo(async () => {
    await pause();
    for (const item of iterable) void item;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

const trapped = new Proxy({} as { value: number }, {
  get() {
    return count();
  },
});
export function Trap() {
  const m = createMemo(async () => {
    await pause();
    void trapped.value;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

class Built {
  constructor() {
    count();
  }
}
class WithField {
  value: unknown = count();
}
export function Constructed() {
  const m = createMemo(async () => {
    await pause();
    new Built();
    new WithField();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

function tag(_strings: TemplateStringsArray) {
  return count();
}
export function Tagged() {
  const m = createMemo(async () => {
    await pause();
    void tag``;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// A helper a member calls runs wherever the member does.
function readsCount() {
  return count();
}
const viaHelper = {
  get value() {
    return readsCount();
  },
};
export function ThroughHelper() {
  const m = createMemo(async () => {
    await pause();
    void viaHelper.value;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Batch 10c (B2): the same routes written as function VALUES. A function stored under a key the host calls by itself (a coercion, the
// promise or iterator protocol, a computed key, a trap of a `new Proxy` handler, the `get` or `set` of a descriptor) runs the same way,
// however it is written: an arrow or a `function`, a name the function is bound to, an assignment, a `defineProperty` descriptor. Each
// read is an uncertifiable row of SC1002 (`function-run-unplaced`), as the method's is above. Each stores its own function.
const coercedArrow = { toString: () => String(count()) };
export function ArrowCoercion() {
  const m = createMemo(async () => {
    await pause();
    void `${coercedArrow}`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const valued = {
  valueOf: function () {
    return count();
  },
};
export function ValueOfFunction() {
  const m = createMemo(async () => {
    await pause();
    void +valued;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const trappedArrow = new Proxy({} as { value: number }, { get: () => count() });
export function ArrowTrap() {
  const m = createMemo(async () => {
    await pause();
    void trappedArrow.value;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const thenable = {
  then: (resolve: (value: number) => void) => {
    resolve(count());
  },
};
export function ArrowThenable() {
  const m = createMemo(async () => {
    await pause();
    await thenable;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const iterableFunction: Iterable<number> = {
  [Symbol.iterator]: function* () {
    count();
    yield 1;
  },
};
export function FunctionIteration() {
  const m = createMemo(async () => {
    await pause();
    for (const item of iterableFunction) void item;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const named = () => String(count());
const byName = { toString: named };
export function NamedFunction() {
  const m = createMemo(async () => {
    await pause();
    void `${byName}`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const assigned: { toString(): string } = {};
assigned.toString = () => String(count());
export function AssignedMember() {
  const m = createMemo(async () => {
    await pause();
    void `${assigned}`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
class Prototyped {}
Prototyped.prototype.toString = function () {
  return String(count());
};
export function PrototypeMember() {
  const m = createMemo(async () => {
    await pause();
    void `${new Prototyped()}`;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
const described = {} as { value: number };
Object.defineProperty(described, "value", { get: () => count() });
export function DescriptorGetter() {
  const m = createMemo(async () => {
    await pause();
    void described.value;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Silent for SC1002: an ordinary handler closure (a call or an event runs it, never the continuation's access), a class no one
// constructs or names (no member of it runs), and a function a call places.
export function Handler() {
  const m = createMemo(async () => {
    await pause();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <button onClick={() => count()}>go</button>;
}
class Never {
  get now() {
    return count();
  }
}
