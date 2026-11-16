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
