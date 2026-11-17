import { onCleanup } from "solid-js";

// ADR 1048: `wrapUseN` holds an `onCleanup`; `fN` calls it in a `try` a `catch` guards or after a loop that a `break` leaves,
// and runs once in the body (owned) and again from a click handler (no owner): NO_OWNER_CLEANUP is proved only where the call
// runs at least once on every completion. At run time `items` is `[0]` (no break), `count` is 1 and `box.value` is defined.
const items: number[] = Math.random() >= 0 ? [0] : [1];
const count = Math.random() >= 0 ? 1 : 0;
const box = { value: 1 };
const noop = () => {};

// GuardedFirst: violation.
function wrapUse1(_value?: number) {
  onCleanup(() => {});
}

export function GuardedFirst() {
  const f1 = () => {
    try {
      wrapUse1();
    } catch {}
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

// GuardedWithFinalizer: violation.
function wrapUse2(_value?: number) {
  onCleanup(() => {});
}

export function GuardedWithFinalizer() {
  const f2 = () => {
    try {
      wrapUse2();
    } catch {
    } finally {
      noop();
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

// GuardedImport: violation.
export function GuardedImport() {
  const f3 = () => {
    try {
      onCleanup(() => {});
    } catch {}
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

// AfterInnerBreak: violation.
function wrapUse4(_value?: number) {
  onCleanup(() => {});
}

export function AfterInnerBreak() {
  const f4 = () => {
    for (const x of items) {
      if (x) break;
    }
    wrapUse4();
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

// AfterInnerSwitch: violation.
function wrapUse5(_value?: number) {
  onCleanup(() => {});
}

export function AfterInnerSwitch() {
  const f5 = () => {
    switch (count) {
      case 1:
        break;
    }
    wrapUse5();
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

// GuardedAfterCall: uncertifiable.
function wrapUse6(_value?: number) {
  onCleanup(() => {});
}

export function GuardedAfterCall() {
  const f6 = () => {
    try {
      noop();
      wrapUse6();
    } catch {}
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

// GuardedConstCallee: uncertifiable.
const arrowUse7 = () => {
  onCleanup(() => {});
};

export function GuardedConstCallee() {
  const f7 = () => {
    try {
      arrowUse7();
    } catch {}
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

// GuardedArgumentThrows: uncertifiable.
function wrapUse8(_value?: number) {
  onCleanup(() => {});
}

export function GuardedArgumentThrows() {
  const f8 = () => {
    try {
      wrapUse8(box.value);
    } catch {}
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

// HandlerArm: uncertifiable.
function wrapUse9(_value?: number) {
  onCleanup(() => {});
}

export function HandlerArm() {
  const f9 = () => {
    try {
      noop();
    } catch {
      wrapUse9();
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

// LabelledBreakOut: uncertifiable.
function wrapUse10(_value?: number) {
  onCleanup(() => {});
}

export function LabelledBreakOut() {
  const f10 = () => {
    outer: {
      for (const x of items) {
        if (x) break outer;
      }
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
