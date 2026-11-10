import { createRoot, createTrackedEffect, flush, onCleanup, onSettled } from "solid-js";

const flag = Math.random() > 0.5;

function wrapUse() {
  flush();
}

// A generator literal stepped once by `.next()` at its own call runs its body
// up to the first `yield` on the leaf's stack: rc.21 raises the code from the
// forbidden call's own line, as for a call written in the callback.
export function Reaches() {
  createTrackedEffect(() => {
    (function* () {
      flush();
      yield 1;
    })().next();
  });
  return <i />;
}
// A helper that first step calls: placed at the helper call.
export function Wrapper() {
  createTrackedEffect(() => {
    (function* () {
      wrapUse();
      yield 1;
    })().next();
  });
  return <i />;
}
// The same in `onSettled`.
export function Settled() {
  onSettled(() => {
    (function* () {
      onCleanup(() => {});
      createRoot(() => 1);
      yield 1;
    })().next();
  });
  return <i />;
}
// A `yield` in one arm may stop the step first: an obligation at the call.
export function MayStop() {
  createTrackedEffect(() => {
    (function* () {
      if (flag) yield 0;
      flush();
    })().next();
  });
  return <i />;
}
// Clean: the one step stops at the first `yield`, before the call.
export function PastYield() {
  createTrackedEffect(() => {
    (function* () {
      yield 1;
      flush();
    })().next();
  });
  return <i />;
}
// Clean: `return()` on a fresh generator runs none of its body.
export function Returned() {
  createTrackedEffect(() => {
    (function* () {
      flush();
      yield 1;
    })().return(undefined);
  });
  return <i />;
}
// A held iterator is no step at its own call: the generator stays an obligation.
export function Held() {
  createTrackedEffect(() => {
    const it = (function* () {
      flush();
      yield 1;
    })();
    it.next();
  });
  return <i />;
}
