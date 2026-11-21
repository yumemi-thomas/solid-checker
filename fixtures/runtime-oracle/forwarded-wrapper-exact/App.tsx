import { createMemo, createTrackedEffect } from "solid-js";
import { drop, ignore, pass } from "./helpers";

// ADR 1179: ADR 0200's forwarding proves a literal (here a pending read, ADR 1155 r3) runs in a wrapper's leaf callback only where the call runs that wrapper exactly: a function
// declaration, a `const` (or a `let` nothing writes) bound to the function itself, an object's member nothing replaces. A `let` written later or a
// call's result is refused (uncertifiable), whichever way the real function goes.

function handDecl(fn: () => void) {
  createTrackedEffect(() => {
    fn();
  });
}
const handConst = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};
let handLet = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};

// Violations (SC5001): the wrapper is exact; Chrome reports PENDING_ASYNC_FORBIDDEN_SCOPE in the leaf.
export function DeclApp() {
  const a1 = createMemo(async () => 1);
  handDecl(() => {
    a1();
  });
  return <i />;
}
export function ConstApp() {
  const a1 = createMemo(async () => 1);
  handConst(() => {
    a1();
  });
  return <i />;
}
export function LetApp() {
  const a1 = createMemo(async () => 1);
  handLet(() => {
    a1();
  });
  return <i />;
}
// An object's method: exact where nothing writes a member of that name (ADR 0279).
const api = {
  hand(fn: () => void) {
    createTrackedEffect(() => {
      fn();
    });
  },
};
export function MemberApp() {
  const a1 = createMemo(async () => 1);
  api.hand(() => {
    a1();
  });
  return <i />;
}

// A data slot holding the declared function by name.
const apiData = { hand: handDecl };
export function MemberDataApp() {
  const a1 = createMemo(async () => 1);
  apiData.hand(() => {
    a1();
  });
  return <i />;
}

// The false-positive twins: the name's first function forwards into the leaf, the function the call runs does not. The read runs bare in
// the component's render (Chrome: ASYNC_OUTSIDE_LOADING_BOUNDARY, not the leaf's code): no SC5001 violation.
let handSwap = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};
handSwap = (fn: () => void) => {
  fn();
};
export function SwapApp() {
  const a1 = createMemo(async () => 1);
  handSwap(() => {
    a1();
  });
  return <i />;
}

const wrapped = ignore((fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
});
export function FactoryApp() {
  const a1 = createMemo(async () => 1);
  wrapped(() => {
    a1();
  });
  return <i />;
}

// The reverse: the function the call runs forwards into the leaf (Chrome: PENDING_ASYNC_FORBIDDEN_SCOPE), but no exact proof says so: uncertifiable, never a claim
// resting on the wrong function.
let swapIn = (fn: () => void) => {
  fn();
};
swapIn = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};
export function SwapInApp() {
  const a1 = createMemo(async () => 1);
  swapIn(() => {
    a1();
  });
  return <i />;
}

const passed = pass((fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
});
export function PassApp() {
  const a1 = createMemo(async () => 1);
  passed(() => {
    a1();
  });
  return <i />;
}

// A member an assignment replaces with a function that runs its argument bare: the declared method is not what runs.
const apiW = {
  run(fn: () => void) {
    createTrackedEffect(() => {
      fn();
    });
  },
};
apiW.run = (fn: () => void) => {
  fn();
};
export function MemberSwapApp() {
  const a1 = createMemo(async () => 1);
  apiW.run(() => {
    a1();
  });
  return <i />;
}

// The false positives main had: the name's first function forwards into the leaf, the function the call runs never calls the literal. Chrome
// is silent; main claimed SC5001. No violation.
let handDrop = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};
handDrop = (fn: () => void) => {
  void fn;
};
export function DropApp() {
  const a1 = createMemo(async () => 1);
  handDrop(() => {
    a1();
  });
  return <i />;
}

const dropped = drop((fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
});
export function FactoryDropApp() {
  const a1 = createMemo(async () => 1);
  dropped(() => {
    a1();
  });
  return <i />;
}

const apiD = {
  skip(fn: () => void) {
    createTrackedEffect(() => {
      fn();
    });
  },
};
apiD.skip = (fn: () => void) => {
  void fn;
};
export function MemberDropApp() {
  const a1 = createMemo(async () => 1);
  apiD.skip(() => {
    a1();
  });
  return <i />;
}

// Data slots the census does not follow (uncertifiable): one naming a `let` an assignment rewrites (its function is not the declaration's),
// one naming a `const` arrow (the census reads a slot's name as a function declaration only, as for shorthand), and one an assignment
// replaces.
const apiSwap = { go: handDrop };
export function DataSwapApp() {
  const a1 = createMemo(async () => 1);
  apiSwap.go(() => {
    a1();
  });
  return <i />;
}

const apiArrow = { go2: handConst };
export function DataArrowApp() {
  const a1 = createMemo(async () => 1);
  apiArrow.go2(() => {
    a1();
  });
  return <i />;
}

const apiReplaced = { go3: handDecl };
apiReplaced.go3 = (fn: () => void) => {
  void fn;
};
export function DataReplacedApp() {
  const a1 = createMemo(async () => 1);
  apiReplaced.go3(() => {
    a1();
  });
  return <i />;
}
