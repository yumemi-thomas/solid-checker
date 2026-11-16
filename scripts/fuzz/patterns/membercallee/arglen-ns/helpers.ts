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
export function argLen(cb: () => unknown) {
  createLoadingBoundary(() => cb(), () => 0);
  return arguments.length;
}
export function mixed(cb: () => unknown) {
  createLoadingBoundary(() => cb(), () => 0);
  Reflect.apply(cb, null, []);
  return 1;
}
export type Tree = { v: number; left?: Tree; right?: Tree };
export function visitBoundary(node: Tree | undefined, cb: () => unknown): void {
  if (!node) {
    createLoadingBoundary(() => cb(), () => 0);
    return;
  }
  visitBoundary(node.left, cb);
  visitBoundary(node.right, cb);
}
export function chain0(cb: () => unknown): number {
  return chain1(cb);
}
export function chain1(cb: () => unknown): number {
  return chain2(cb);
}
export function chain2(cb: () => unknown): number {
  return chain3(cb);
}
export function chain3(cb: () => unknown): number {
  return chain4(cb);
}
export function chain4(cb: () => unknown): number {
  return chain5(cb);
}
export function chain5(cb: () => unknown): number {
  return chain6(cb);
}
export function chain6(cb: () => unknown): number {
  return chain7(cb);
}
export function chain7(cb: () => unknown): number {
  return chain8(cb);
}
export function chain8(cb: () => unknown): number {
  return chain9(cb);
}
export function chain9(cb: () => unknown): number {
  return chain10(cb);
}
export function chain10(cb: () => unknown): number {
  return chain11(cb);
}
export function chain11(cb: () => unknown): number {
  return chain12(cb);
}
export function chain12(cb: () => unknown): number {
  return chain13(cb);
}
export function chain13(cb: () => unknown): number {
  return chain14(cb);
}
export function chain14(cb: () => unknown): number {
  return chain15(cb);
}
export function chain15(cb: () => unknown): number {
  return chain16(cb);
}
export function chain16(cb: () => unknown): number {
  return lastBare(cb);
}
function lastBare(cb: () => unknown) {
  cb();
  return 1;
}
