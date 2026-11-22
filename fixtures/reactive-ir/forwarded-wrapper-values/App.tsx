import { createMemo, createTrackedEffect } from "solid-js";
import { either, noted } from "./helpers";

// ADR 1182: the value a wrapper's name holds at a call, where it is not the function first written for it. Each wrapper's first or handed
// function forwards a pending read into createTrackedEffect's callback; what runs is decided only where every value is proved. Here none is
// (uncertifiable) but `LateApp`; `forwarded-wrapper-exact` holds the proved shapes.

const leaf = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};
const bare = (fn: () => void) => {
  fn();
};
let flag = false;

// A write in a function: when it runs is not module order.
let inFunction = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};
export function reset() {
  inFunction = (fn: () => void) => {
    fn();
  };
}
export function InFunctionApp() {
  const a1 = createMemo(async () => 1);
  inFunction(() => {
    a1();
  });
  return <i />;
}

// A write on a branch.
let onBranch = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};
if (flag) {
  onBranch = (fn: () => void) => {
    fn();
  };
}
export function OnBranchApp() {
  const a1 = createMemo(async () => 1);
  onBranch(() => {
    a1();
  });
  return <i />;
}

// A write of a value that is no function literal.
let byName = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};
byName = bare;
export function ByNameApp() {
  const a1 = createMemo(async () => 1);
  byName(() => {
    a1();
  });
  return <i />;
}

// A call module code makes before the last write sees the first function (`early`'s, uncertifiable); the component's call runs at
// render, after it, and sees the leaf wrapper (`LateApp`, a violation: the proof that the call follows the last write).
let late = (fn: () => void) => {
  fn();
};
function early() {
  const a0 = createMemo(async () => 0);
  late(() => {
    a0();
  });
}
// Its pending read throws during module evaluation; caught, so the module goes on to the write and the components mount.
try {
  early();
} catch {
  void 0;
}
late = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};
export function LateApp() {
  const a1 = createMemo(async () => 1);
  late(() => {
    a1();
  });
  return <i />;
}

// A factory that runs code before it returns, and one whose return is conditional.
const made = noted((fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
});
export function NotedApp() {
  const a1 = createMemo(async () => 1);
  made(() => {
    a1();
  });
  return <i />;
}
const chosen = either(leaf, flag);
export function EitherApp() {
  const a1 = createMemo(async () => 1);
  chosen(() => {
    a1();
  });
  return <i />;
}

// A member written in a function, and a receiver exported (another module may write it).
const apiF = {
  run(fn: () => void) {
    leaf(fn);
  },
};
export function patch() {
  apiF.run = (fn: () => void) => {
    fn();
  };
}
export function MemberInFunctionApp() {
  const a1 = createMemo(async () => 1);
  apiF.run(() => {
    a1();
  });
  return <i />;
}
export const apiE = {
  go(fn: () => void) {
    void fn;
  },
};
apiE.go = (fn: () => void) => {
  createTrackedEffect(() => {
    fn();
  });
};
export function MemberExportedApp() {
  const a1 = createMemo(async () => 1);
  apiE.go(() => {
    a1();
  });
  return <i />;
}

void flag;
flag = true;
