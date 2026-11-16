import { createSignal } from "solid-js";

// ADR 1014, the review's fourth probe: a function stored under ANOTHER member of the holder after the literal, which hands its receiver on and so replaces
// the member the timer is handed. The function the timer calls is the replacement, a `function` that reads its `this` (the window): the checker refuses
// each (the slots are withdrawn, `tuple-iteration-written`) and proves nothing about the write below. Chrome rc.21 still reports the write (nothing here
// patches iteration): covered by the uncertifiable row, never by a violation.
const seen: unknown[] = [];

// (1) a method added after the literal hands its receiver to `Object.assign`.
const added: { cb: (this: unknown) => void; init?: () => void } = { cb: () => {} };
added.init = function () {
  Object.assign(this, {
    cb(this: unknown) {
      seen.push(this);
    },
  });
};
added.init();
setTimeout(added.cb, 0);

// (2) the same with `Object.defineProperty`.
const defined: { cb: (this: unknown) => void; init?: () => void } = { cb: () => {} };
defined.init = function () {
  Object.defineProperty(this, "cb", {
    value(this: unknown) {
      seen.push(this);
    },
  });
};
defined.init();
setTimeout(defined.cb, 0);

// (3) the method added is another object's.
const other = {
  f(this: object) {
    Object.assign(this, {
      cb(this: unknown) {
        seen.push(this);
      },
    });
  },
};
const borrowed: { cb: (this: unknown) => void; init?: (this: object) => void } = { cb: () => {} };
borrowed.init = other.f;
borrowed.init();
setTimeout(borrowed.cb, 0);

// (4) a method of the literal stores the function under another member through `this`.
const stored: { cb: (this: unknown) => void; later?: () => void; init(): void } = {
  cb: () => {},
  init() {
    this.later = function () {
      Object.assign(this, {
        cb(this: unknown) {
          seen.push(this);
        },
      });
    };
  },
};
stored.init();
stored.later?.();
setTimeout(stored.cb, 0);

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}

// What the timers handed their callbacks: read after they fired, the four receivers are the window.
export function Receivers() {
  const [n, setN] = createSignal(-1);
  setTimeout(() => setN(seen.filter((value) => value === window).length), 30);
  return <output data-receivers>{n()}</output>;
}
