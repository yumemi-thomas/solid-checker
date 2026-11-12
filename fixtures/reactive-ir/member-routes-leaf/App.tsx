import { createMemo, flush, onCleanup, onSettled } from "solid-js";

// ADR 0963 (batch 10b, group A): the callback of a leaf owner (`onSettled`, `createTrackedEffect`) runs a member without a call:
// a property access runs a getter, a coercion `toString`, a `for...of` or a spread `[Symbol.iterator]`, a Proxy access its trap,
// `new` a constructor and the class's field initializers, a tagged template its tag. The leaf walk follows calls and runs none
// of these, so a creation, a cleanup or a flush there throws in dev (Chrome, rc.21: PRIMITIVE_IN_FORBIDDEN_SCOPE,
// CLEANUP_IN_FORBIDDEN_SCOPE, FLUSH_IN_TRACKED_QUEUE_CALLBACK) while the program was certified clean. Each such call below is
// an uncertifiable row of SC3001 (`function-run-unplaced`, or `field-initializer-unplaced` directly in a field's initializer),
// never a violation and never silence, while the program holds a leaf owner.

const accessors = {
  get value() {
    createMemo(() => 1);
    return 1;
  },
};
export function ObjectAccessor() {
  onSettled(() => {
    void accessors.value;
  });
  return <div />;
}

const coerced = {
  toString() {
    onCleanup(() => {});
    return "";
  },
};
export function Coercion() {
  onSettled(() => {
    void `${coerced}`;
  });
  return <div />;
}

const iterable = {
  [Symbol.iterator]() {
    createMemo(() => 1);
    return [1][Symbol.iterator]();
  },
};
export function Iteration() {
  onSettled(() => {
    for (const item of iterable) void item;
  });
  return <div />;
}

const trapped = new Proxy({} as { value: number }, {
  get() {
    flush();
    return 1;
  },
});
export function Trap() {
  onSettled(() => {
    void trapped.value;
  });
  return <div />;
}

class Built {
  constructor() {
    createMemo(() => 1);
  }
}
class WithField {
  value: unknown = createMemo(() => 1);
}
function Legacy(this: unknown) {
  createMemo(() => 1);
}
export function Constructed() {
  onSettled(() => {
    new Built();
    new WithField();
    new (Legacy as unknown as new () => object)();
  });
  return <div />;
}

function tag(_strings: TemplateStringsArray) {
  createMemo(() => 1);
  return 1;
}
export function Tagged() {
  onSettled(() => {
    void tag``;
  });
  return <div />;
}

// Batch 10c (B2): the same routes written as function values (an arrow or a `function`, a name the function is bound to, an assignment, a
// `defineProperty` descriptor, a Proxy trap written as an arrow): the key decides which are run by the host (see `member-routes-after-await`).
const coercedArrow = {
  toString: () => {
    createMemo(() => 1);
    return "";
  },
};
export function ArrowCoercion() {
  onSettled(() => {
    void `${coercedArrow}`;
  });
  return <div />;
}
const trappedArrow = new Proxy({} as { value: number }, {
  get: () => {
    onCleanup(() => {});
    return 1;
  },
});
export function ArrowTrap() {
  onSettled(() => {
    void trappedArrow.value;
  });
  return <div />;
}
const assigned: { toString(): string } = {};
assigned.toString = () => {
  createMemo(() => 1);
  return "";
};
export function AssignedMember() {
  onSettled(() => {
    void `${assigned}`;
  });
  return <div />;
}
const named = () => {
  flush();
  return "";
};
const byName = { toString: named };
export function NamedFunction() {
  onSettled(() => {
    void `${byName}`;
  });
  return <div />;
}
const described = {} as { value: number };
Object.defineProperty(described, "value", {
  get: () => {
    createMemo(() => 1);
    return 1;
  },
});
export function DescriptorGetter() {
  onSettled(() => {
    void described.value;
  });
  return <div />;
}

// Unchanged: a creation written in the leaf callback is the leaf walk's own violation, and a helper the leaf calls is read through
// its call (the violation stands at the call). Silent: a function a call places and no leaf runs.
export function Direct() {
  onSettled(() => {
    createMemo(() => 1);
  });
  return <div />;
}
function make() {
  createMemo(() => 1);
}
export function PlainHelper() {
  onSettled(() => {
    make();
  });
  return <div />;
}
function placed() {
  createMemo(() => 1);
}
export function Placed() {
  placed();
  return <div />;
}
