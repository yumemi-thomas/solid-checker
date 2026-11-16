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

// Callees the review of the third cut asked for (ADR 1011). A closure whose body reads `arguments.length` beside a boundary: the walk cannot read the
// body (`arguments`), a member callee stays uncertifiable (Chrome is quiet), an identifier callee keeps main's answer (not pinned: main reports a violation
// there that Chrome does not: `docs/open-work.md`).
export function argLen(cb: () => unknown) {
  createLoadingBoundary(() => cb(), () => 0);
  return arguments.length;
}

// One use held by a boundary and one the walk cannot place (`Reflect.apply`): the closure may be held, uncertifiable.
export function mixed(cb: () => unknown) {
  createLoadingBoundary(() => cb(), () => 0);
  Reflect.apply(cb, null, []);
  return 1;
}

// A recursive visitor whose base case is the boundary: the parameter reaches itself and a boundary, uncertifiable (pinned).
export type Tree = { v: number; left?: Tree; right?: Tree };
export function visitBoundary(node: Tree | undefined, cb: () => unknown): void {
  if (!node) {
    createLoadingBoundary(() => cb(), () => 0);
    return;
  }
  visitBoundary(node.left, cb);
  visitBoundary(node.right, cb);
}

// The chain of hand-offs the walk follows is sixteen long (ADR 0988's bound): seventeen functions hand the closure on and the last calls it bare.
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
