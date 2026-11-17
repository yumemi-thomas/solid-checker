import { createSignal, onCleanup } from "solid-js";

// ADR 1042: `wrapUseN` holds an `onCleanup`; `fN` calls it under one form and runs once in the body (owned) and again from a
// click handler (no owner): NO_OWNER_CLEANUP is proved only where the call runs at least once on every run of `fN`.
// `maybe` is false at run time and decided by no fact.
const maybe = Math.random() > 2;
const noop = () => {};

// Label: violation.
function wrapUse1() {
  onCleanup(() => {});
}

export function Label() {
  const f1 = () => {
    outer: {
      wrapUse1();
      break outer;
    }
  };
  f1();
  document.body.removeEventListener("click", f1);
  return (
    <div>
      <button data-act="b1" onClick={() => {
        [1].forEach(f1);
      }}>go</button>
    </div>
  );
}

// Finalizer: violation.
function wrapUse2() {
  onCleanup(() => {});
}

export function Finalizer() {
  const f2 = () => {
    try {
    } finally {
      wrapUse2();
    }
  };
  f2();
  document.body.removeEventListener("click", f2);
  return (
    <div>
      <button data-act="b2" onClick={() => {
        [1].forEach(f2);
      }}>go</button>
    </div>
  );
}

// TryNoCatch: violation.
function wrapUse3() {
  onCleanup(() => {});
}

export function TryNoCatch() {
  const f3 = () => {
    try {
      wrapUse3();
    } finally {
    }
  };
  f3();
  document.body.removeEventListener("click", f3);
  return (
    <div>
      <button data-act="b3" onClick={() => {
        [1].forEach(f3);
      }}>go</button>
    </div>
  );
}

// DoBody: violation.
function wrapUse4() {
  onCleanup(() => {});
}

export function DoBody() {
  const f4 = () => {
    do {
      wrapUse4();
    } while (false);
  };
  f4();
  document.body.removeEventListener("click", f4);
  return (
    <div>
      <button data-act="b4" onClick={() => {
        [1].forEach(f4);
      }}>go</button>
    </div>
  );
}

// WhileTrue: violation.
function wrapUse5() {
  onCleanup(() => {});
}

export function WhileTrue() {
  const f5 = () => {
    while (true) {
      wrapUse5();
      break;
    }
  };
  f5();
  document.body.removeEventListener("click", f5);
  return (
    <div>
      <button data-act="b5" onClick={() => {
        [1].forEach(f5);
      }}>go</button>
    </div>
  );
}

// ForNoTest: violation.
function wrapUse6() {
  onCleanup(() => {});
}

export function ForNoTest() {
  const f6 = () => {
    for (;;) {
      wrapUse6();
      break;
    }
  };
  f6();
  document.body.removeEventListener("click", f6);
  return (
    <div>
      <button data-act="b6" onClick={() => {
        [1].forEach(f6);
      }}>go</button>
    </div>
  );
}

// LabelledLoop: violation.
function wrapUse7() {
  onCleanup(() => {});
}

export function LabelledLoop() {
  const f7 = () => {
    outer: while (true) {
      wrapUse7();
      break outer;
    }
  };
  f7();
  document.body.removeEventListener("click", f7);
  return (
    <div>
      <button data-act="b7" onClick={() => {
        [1].forEach(f7);
      }}>go</button>
    </div>
  );
}

// CatchAndFinalizer: violation.
function wrapUse8() {
  onCleanup(() => {});
}

export function CatchAndFinalizer() {
  const f8 = () => {
    try {
      noop();
    } catch {
    } finally {
      wrapUse8();
    }
  };
  f8();
  document.body.removeEventListener("click", f8);
  return (
    <div>
      <button data-act="b8" onClick={() => {
        [1].forEach(f8);
      }}>go</button>
    </div>
  );
}

// LabelInFinalizer: violation.
function wrapUse9() {
  onCleanup(() => {});
}

export function LabelInFinalizer() {
  const f9 = () => {
    try {
    } finally {
      inner: {
        wrapUse9();
      }
    }
  };
  f9();
  document.body.removeEventListener("click", f9);
  return (
    <div>
      <button data-act="b9" onClick={() => {
        [1].forEach(f9);
      }}>go</button>
    </div>
  );
}

// EarlyBreakOuter: uncertifiable.
function wrapUse10() {
  onCleanup(() => {});
}

export function EarlyBreakOuter() {
  const f10 = () => {
    outer: {
      if (maybe) break outer;
      wrapUse10();
    }
  };
  f10();
  document.body.removeEventListener("click", f10);
  return (
    <div>
      <button data-act="b10" onClick={() => {
        [1].forEach(f10);
      }}>go</button>
    </div>
  );
}

// HandlerArm: uncertifiable.
function wrapUse11() {
  onCleanup(() => {});
}

export function HandlerArm() {
  const f11 = () => {
    try {
      noop();
    } catch {
      wrapUse11();
    }
  };
  f11();
  document.body.removeEventListener("click", f11);
  return (
    <div>
      <button data-act="b11" onClick={() => {
        [1].forEach(f11);
      }}>go</button>
    </div>
  );
}

// FinalizerEarlyReturn: uncertifiable.
function wrapUse12() {
  onCleanup(() => {});
}

export function FinalizerEarlyReturn() {
  const f12 = () => {
    try {
    } finally {
      if (maybe) return;
      wrapUse12();
    }
  };
  f12();
  document.body.removeEventListener("click", f12);
  return (
    <div>
      <button data-act="b12" onClick={() => {
        [1].forEach(f12);
      }}>go</button>
    </div>
  );
}

// DoBreakFirst: uncertifiable.
function wrapUse13() {
  onCleanup(() => {});
}

export function DoBreakFirst() {
  const f13 = () => {
    do {
      if (maybe) break;
      wrapUse13();
    } while (maybe);
  };
  f13();
  document.body.removeEventListener("click", f13);
  return (
    <div>
      <button data-act="b13" onClick={() => {
        [1].forEach(f13);
      }}>go</button>
    </div>
  );
}

// WhileCondition: uncertifiable.
function wrapUse14() {
  onCleanup(() => {});
}

export function WhileCondition() {
  const f14 = () => {
    while (!maybe) {
      wrapUse14();
      break;
    }
  };
  f14();
  document.body.removeEventListener("click", f14);
  return (
    <div>
      <button data-act="b14" onClick={() => {
        [1].forEach(f14);
      }}>go</button>
    </div>
  );
}

// WhileContinueFirst: uncertifiable.
function wrapUse15() {
  onCleanup(() => {});
}

export function WhileContinueFirst() {
  const f15 = () => {
    while (true) {
      if (maybe) continue;
      wrapUse15();
      break;
    }
  };
  f15();
  document.body.removeEventListener("click", f15);
  return (
    <div>
      <button data-act="b15" onClick={() => {
        [1].forEach(f15);
      }}>go</button>
    </div>
  );
}

// ForOfBody: uncertifiable.
function wrapUse16() {
  onCleanup(() => {});
}

export function ForOfBody() {
  const f16 = () => {
    for (const x of [1]) {
      wrapUse16();
    }
  };
  f16();
  document.body.removeEventListener("click", f16);
  return (
    <div>
      <button data-act="b16" onClick={() => {
        [1].forEach(f16);
      }}>go</button>
    </div>
  );
}

// CatchGuardedBlock: uncertifiable.
function wrapUse17() {
  onCleanup(() => {});
}

export function CatchGuardedBlock() {
  const f17 = () => {
    try {
      noop();
      wrapUse17();
    } catch {
    }
  };
  f17();
  document.body.removeEventListener("click", f17);
  return (
    <div>
      <button data-act="b17" onClick={() => {
        [1].forEach(f17);
      }}>go</button>
    </div>
  );
}

// SC1001, uncertifiable: a read reached through a function summary under a label is placed by the Type Facts producer's
// exactly-once whitelist, which ADR 1042 leaves unchanged.
const [count] = createSignal(0);
const read = () => count();

export function LabelRead() {
  const r = () => {
    outer: {
      read();
      break outer;
    }
  };
  r();
  return <div></div>;
}
