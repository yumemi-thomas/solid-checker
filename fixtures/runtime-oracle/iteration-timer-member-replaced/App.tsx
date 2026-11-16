import { createSignal } from "solid-js";

// ADR 1014, the review's holes: three ways a member of a `const` object literal is replaced with no assignment that names it, each followed by a timer handed
// that member. The function the timer calls is the replacement, a `function` that reads its `this` (the window), so the member is no proof that the window
// reaches no code: the checker refuses each (the slots are withdrawn, `tuple-iteration-written`) and proves nothing about the write below. Chrome rc.21 still
// reports the write (nothing here patches iteration): covered by the uncertifiable row, never by a violation.
const seen: unknown[] = [];

// (1) a write through a key no literal and no constant names.
const computed: { cb: (this: unknown) => void } = { cb: () => {} };
const key = ["cb"][0] as "cb";
computed[key] = function (this: unknown) {
  seen.push(this);
};
setTimeout(computed.cb, 0);

// (2) a method of the literal hands its receiver to `Object.assign`.
const assigned = {
  cb: (() => {}) as (this: unknown) => void,
  init() {
    Object.assign(this, {
      cb(this: unknown) {
        seen.push(this);
      },
    });
  },
};
assigned.init();
setTimeout(assigned.cb, 0);

// (2b) a method of the literal hands its receiver to `Object.defineProperty`.
const defined = {
  cb: (() => {}) as (this: unknown) => void,
  init() {
    Object.defineProperty(this, "cb", {
      value(this: unknown) {
        seen.push(this);
      },
    });
  },
};
defined.init();
setTimeout(defined.cb, 0);

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}

// What the timers handed their callbacks: read after they fired, the three receivers are the window.
export function Receivers() {
  const [n, setN] = createSignal(-1);
  setTimeout(() => setN(seen.filter((value) => value === window).length), 30);
  return <output data-receivers>{n()}</output>;
}
