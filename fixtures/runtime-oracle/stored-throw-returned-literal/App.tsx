import { dynamic } from "@solidjs/web";
import { boxedElsewhere } from "./box";

// A static `dynamic` source that is a promise throws where the call runs (DYNAMIC_STATIC_PROMISE, SC2007, a plain throw, ADR 0893).
// ADR 1058: a helper that returns a fresh object literal holding its parameter (`return { cb }`) boxes the caller's function, so a
// closed `const box = boxed(f)` is that literal: `box.cb()` runs `f` on its own stack (ADR 1044's closed-literal member).
function boxed(cb: () => void) {
  return { cb };
}
function keyed(cb: () => void) {
  return { cb: cb, size: 1 };
}

// Violations: the component body runs the throw.
export function BoxedCalled() {
  const box = boxed(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  box.cb();
  return <p />;
}
export function KeyedCalled() {
  const box = keyed(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  box.cb();
  return <p>{box.size}</p>;
}

// Obligations (uncertifiable): the box is not a closed literal, or the call is caught.
let saved: (() => void) | undefined;
function storesToo(cb: () => void) {
  saved = cb;
  return { cb };
}
export function HelperAlsoStores() {
  const box = storesToo(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  box.cb();
  void saved;
  return <p />;
}
function either(cb: () => void, flag: boolean) {
  if (flag) return { cb };
  return { cb: () => {} };
}
export function TwoReturns() {
  const box = either(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  }, true);
  box.cb();
  return <p />;
}
function spreads(cb: () => void, rest: { cb?: () => void }) {
  return { ...rest, cb };
}
export function HelperSpreads() {
  const box = spreads(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  }, {});
  box.cb();
  return <p />;
}
function hand(box: { cb: () => void }) {
  box.cb = () => {};
}
export function BoxHandedOn() {
  const box = boxed(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  hand(box);
  box.cb();
  return <p />;
}
export function BoxWritten() {
  const box = boxed(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  box.cb = () => {};
  box.cb();
  return <p />;
}
export function BoxCaught() {
  const box = boxed(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  try {
    box.cb();
  } catch {
    // swallowed
  }
  return <p />;
}
export function ImportedHelper() {
  const box = boxedElsewhere(() => {
    dynamic(() => Promise.resolve("p" as const), { static: true });
  });
  box.cb();
  return <p />;
}
