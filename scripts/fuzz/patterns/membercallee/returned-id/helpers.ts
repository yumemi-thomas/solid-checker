import { createLoadingBoundary } from "solid-js/internal";
export function run(cb: () => unknown) {
  cb();
  return 1;
}
export const kept: Array<() => unknown> = [];
export function keep(cb: () => unknown) {
  kept.push(cb);
  return 1;
}
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
export function later(cb: () => unknown) {
  queueMicrotask(cb);
  return 1;
}
export function maybe(cb: () => unknown, on: boolean) {
  if (on) cb();
  return 1;
}
export function passOn(cb: () => unknown) {
  return run(cb);
}
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
export function returned(cb: () => unknown) {
  createLoadingBoundary(() => cb(), () => 0);
  return () => cb();
}
export function viaArguments(cb: () => unknown) {
  createLoadingBoundary(() => cb(), () => 0);
  return arguments[0]();
}
export function ignore(_inner: (cb: () => unknown) => number) {
  return (cb: () => unknown) => {
    cb();
    return 0;
  };
}
