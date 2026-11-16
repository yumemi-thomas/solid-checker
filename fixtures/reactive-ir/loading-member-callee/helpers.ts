import { createLoadingBoundary } from "solid-js/internal";

// Project functions the components hand a closure to. `run` calls it at once (a render-time call); `keep` stores it.
export function run(cb: () => unknown) {
  cb();
  return 1;
}

export const kept: Array<() => unknown> = [];
export function keep(cb: () => unknown) {
  kept.push(cb);
  return 1;
}

// A callee that puts a boundary around its calls of the parameter: Chrome is quiet at a read in the closure.
export function exprArrow(cb: () => unknown) {
  createLoadingBoundary(() => cb(), () => 0);
  return 1;
}

export function inBoundary(cb: () => unknown) {
  createLoadingBoundary(() => {
    cb();
    return 1;
  }, () => 0);
  return 1;
}

// A callee that runs the closure later, or only when a flag says so.
export function later(cb: () => unknown) {
  queueMicrotask(cb);
  return 1;
}

export function maybe(cb: () => unknown, on: boolean) {
  if (on) cb();
  return 1;
}

// A callee that passes the closure on to a function that calls it.
export function passOn(cb: () => unknown) {
  return run(cb);
}

// A callee that hands the closure down a chain: the first is held by no boundary (Chrome warns), the second ends in one (Chrome is quiet).
export function forward(cb: () => unknown) {
  return inner(cb);
}
function inner(cb: () => unknown) {
  cb();
  return 1;
}
export function forwardBoundary(cb: () => unknown) {
  return innerBoundary(cb);
}
function innerBoundary(cb: () => unknown) {
  createLoadingBoundary(() => cb(), () => 0);
  return 1;
}

// Callees whose body puts a boundary around SOME of its uses of the parameter and runs it bare in another (Chrome warns at the read: the bare
// call runs in the render): a violation must stay, never the open read a boundary somewhere would make of it.
export function both(cb: () => unknown) {
  cb();
  createLoadingBoundary(() => cb(), () => 0);
  return 1;
}

export function branch(cb: () => unknown, held: boolean) {
  if (held) {
    createLoadingBoundary(() => cb(), () => 0);
  } else {
    cb();
  }
  return 1;
}

// The boundary holds one call; the closure returned runs it bare when the caller calls the result.
export function returned(cb: () => unknown) {
  createLoadingBoundary(() => cb(), () => 0);
  return () => cb();
}

// `arguments[0]` runs the parameter without naming it.
export function viaArguments(cb: () => unknown) {
  createLoadingBoundary(() => cb(), () => 0);
  return arguments[0]();
}

// What `wrapped` is, is the function `ignore` returns (it runs the closure bare), not the function written in the call.
export function ignore(_inner: (cb: () => unknown) => number) {
  return (cb: () => unknown) => {
    cb();
    return 0;
  };
}
