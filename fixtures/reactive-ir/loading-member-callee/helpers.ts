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
