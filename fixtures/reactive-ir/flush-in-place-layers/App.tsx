import { action, createEffect, createSignal, createTrackedEffect, flush } from "solid-js";

const [n] = createSignal(0);

// ADR 1090: the program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason holds and each flush stands on where it is
// placed. A flush written in exact in-place layers inside a timer's callback (a local closure called once, a named helper called once, an IIFE) runs on the
// callback's stack: the layers are placed at their outermost call, which is written directly in the callback, as a flush written there is.
export const step = action(function* () {
  yield 1;
});

export function Host() {
  createEffect(() => n(), () => {});
  createTrackedEffect(() => {
    n();
  });
  return <div />;
}

// Clean: a local closure called once in the callback.
export function Closure() {
  setTimeout(() => {
    const f1 = () => {
      flush();
    };
    f1();
  }, 0);
  return <div />;
}

// Clean: a module helper called once, in the callback only.
function runFlush() {
  flush();
}
export function Helper() {
  setTimeout(() => {
    runFlush();
  }, 0);
  return <div />;
}

// Clean: an IIFE in the callback.
export function Iife() {
  setTimeout(() => {
    (() => {
      flush();
    })();
  }, 0);
  return <div />;
}

// Uncertifiable (fail closed): the closure is also handed on, so a run of it may be elsewhere (an action's step here).
const kept: (() => void)[] = [];
export function HandedOn() {
  setTimeout(() => {
    const f2 = () => {
      flush();
    };
    kept.push(f2);
    f2();
  }, 0);
  return <div />;
}

// Uncertifiable (fail closed): the helper is called from the timer and from an action's step.
function sharedFlush() {
  flush();
}
export const stepShared = action(function* () {
  sharedFlush();
  yield 1;
});
export function Shared() {
  setTimeout(() => {
    sharedFlush();
  }, 0);
  return <div />;
}
