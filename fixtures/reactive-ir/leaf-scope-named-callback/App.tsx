import { createMemo, createRenderEffect, createSignal, createTrackedEffect, onSettled } from "solid-js";

// A function a leaf primitive names by its binding runs under the leaf owner
// the write guard exempts, as the inline callback does.

function run(cb: () => unknown) {
  cb();
  return 1;
}

// Negative: a const arrow named at createTrackedEffect.
export function Const() {
  const [, setCount] = createSignal(0);
  const cb = () => {
    setCount(7);
  };
  createTrackedEffect(cb);
  return <div />;
}

// Negative: a function declaration named at createTrackedEffect.
export function FunctionDeclaration() {
  const [, setCount] = createSignal(0);
  function cb() {
    setCount(7);
  }
  createTrackedEffect(cb);
  return <div />;
}

// Negative: a let binding that is never reassigned.
export function Let() {
  const [, setCount] = createSignal(0);
  let cb = () => {
    setCount(7);
  };
  createTrackedEffect(cb);
  return <div />;
}

// Negative: a named function expression under its binding (ADR 0515).
export function NamedExpression() {
  const [, setCount] = createSignal(0);
  const cb = function step() {
    setCount(7);
  };
  createTrackedEffect(cb);
  return <div />;
}

// Negative: the same function handed to two tracked effects.
export function SharedTwice() {
  const [, setCount] = createSignal(0);
  const cb = () => {
    setCount(7);
  };
  createTrackedEffect(cb);
  createTrackedEffect(cb);
  return <div />;
}

// Negative: named at onSettled.
export function Settled() {
  const [, setCount] = createSignal(0);
  const cb = () => {
    setCount(7);
  };
  onSettled(cb);
  return <div />;
}

// Negative: the write sits in a helper's callback the named leaf callback runs
// in place (ADR 0492's climb).
export function HelperInside() {
  const [, setCount] = createSignal(0);
  const cb = () => {
    run(() => {
      setCount(7);
    });
  };
  createTrackedEffect(cb);
  return <div />;
}

// Violation: the same function named as a memo's compute.
export function MemoNamed() {
  const [, setCount] = createSignal(0);
  const cb = () => {
    setCount(7);
    return 1;
  };
  createMemo(cb);
  return <div />;
}

// Violation: named at a tracked effect and at a memo; the memo entry rejects.
export function SharedWithMemo() {
  const [, setCount] = createSignal(0);
  const cb = () => {
    setCount(7);
  };
  createTrackedEffect(cb);
  createMemo(cb);
  return <div />;
}

// Violation: a render effect's compute named by its binding (ADR 0466).
export function RenderComputeNamed() {
  const [, setCount] = createSignal(0);
  const cb = () => {
    setCount(7);
    return 1;
  };
  createRenderEffect(cb, () => {});
  return <div />;
}
