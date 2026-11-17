import { onCleanup } from "solid-js";

// ADR 1046: `wrapUseN` holds an `onCleanup`; `fN` calls it in a counter loop or a literal `switch` and runs once in the body
// (owned) and again from a click handler (no owner): NO_OWNER_CLEANUP is proved only where literals show the call runs.
// `maybe` is false and `count` is 1 at run time; no fact decides either.
const maybe = Math.random() > 2;
const count = Math.random() >= 0 ? 1 : 0;
const noop = () => {};

// CounterLoop: violation.
function wrapUse1() {
  onCleanup(() => {});
}

export function CounterLoop() {
  const f1 = () => {
    for (let i = 0; i < 1; i += 1) {
      wrapUse1();
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

// CounterThree: violation.
function wrapUse2() {
  onCleanup(() => {});
}

export function CounterThree() {
  const f2 = () => {
    for (let i = 0; i < 3; i++) {
      wrapUse2();
      break;
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

// CountDown: violation.
function wrapUse3() {
  onCleanup(() => {});
}

export function CountDown() {
  const f3 = () => {
    for (let i = 2; i > 0; i -= 1) {
      wrapUse3();
      break;
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

// LiteralSwitch: violation.
function wrapUse4() {
  onCleanup(() => {});
}

export function LiteralSwitch() {
  const f4 = () => {
    switch (1 as number) {
      case 1: {
        wrapUse4();
        break;
      }
    }
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

// StringSwitch: violation.
function wrapUse5() {
  onCleanup(() => {});
}

export function StringSwitch() {
  const f5 = () => {
    switch ("b" as string) {
      case "a":
        noop();
        break;
      case "b":
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

// ZeroIterations: uncertifiable.
function wrapUse6() {
  onCleanup(() => {});
}

export function ZeroIterations() {
  const f6 = () => {
    for (let i = 0; i < 0; i += 1) {
      wrapUse6();
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

// BreakFirst: uncertifiable.
function wrapUse7() {
  onCleanup(() => {});
}

export function BreakFirst() {
  const f7 = () => {
    for (let i = 0; i < 1; i += 1) {
      if (maybe) break;
      wrapUse7();
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

// ContinueFirst: uncertifiable.
function wrapUse8() {
  onCleanup(() => {});
}

export function ContinueFirst() {
  const f8 = () => {
    for (let i = 0; i < 1; i += 1) {
      if (maybe) continue;
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

// BoundNotLiteral: uncertifiable.
function wrapUse9() {
  onCleanup(() => {});
}

export function BoundNotLiteral() {
  const f9 = () => {
    for (let i = 0; i < count; i += 1) {
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

// SwitchNotLiteral: uncertifiable.
function wrapUse10() {
  onCleanup(() => {});
}

export function SwitchNotLiteral() {
  const f10 = () => {
    switch (count) {
      case 1:
        wrapUse10();
        break;
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

// SwitchMiss: uncertifiable.
function wrapUse11() {
  onCleanup(() => {});
}

export function SwitchMiss() {
  const f11 = () => {
    switch (2 as number) {
      case 1:
        wrapUse11();
        break;
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

// FallThrough: uncertifiable.
function wrapUse12() {
  onCleanup(() => {});
}

export function FallThrough() {
  const f12 = () => {
    switch (1 as number) {
      case 1:
        noop();
      case 2:
        wrapUse12();
        break;
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

// DefaultArm: uncertifiable.
function wrapUse13() {
  onCleanup(() => {});
}

export function DefaultArm() {
  const f13 = () => {
    switch (1 as number) {
      default:
        wrapUse13();
    }
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

// CaseBreakFirst: uncertifiable.
function wrapUse14() {
  onCleanup(() => {});
}

export function CaseBreakFirst() {
  const f14 = () => {
    switch (1 as number) {
      case 1:
        if (maybe) break;
        wrapUse14();
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
