import { action, createEffect, createSignal, onSettled } from "solid-js";

// ADR 1051 (SC2008): an action whose first step stops at a `yield` of a value that is no thenable steps again at once
// inside `inTransaction`, which flushes when no action step is on the stack (`@solidjs/signals@2.0.0-rc.21`
// `dist/dev.js:2198-2208`). Called from a `createEffect` apply, that flush runs inside the flush draining effects: it
// drains nothing, and the dev build warns FLUSH_IN_EFFECT_CALLBACK at the action's call.
const [, setCount] = createSignal(0);

// Proved: the first statement is `yield 1`.
export function YieldNumber() {
  const a1 = action(function* () {
    yield 1;
    setCount(1);
  });
  createEffect(() => 1, () => {
    a1();
  });
  return <div />;
}

// Proved: a bare `yield` (it yields `undefined`).
export function BareYield() {
  const a2 = action(function* () {
    yield;
    setCount(2);
  });
  createEffect(() => 1, () => {
    a2();
  });
  return <div />;
}

// Proved: `yield "step"`, the action called with an argument.
export function YieldString() {
  const a3 = action(function* (n: number) {
    yield "step";
    setCount(n);
  });
  createEffect(() => 1, () => {
    a3(3);
  });
  return <div />;
}

// No row: a promise is assimilated, and the next step (with its flush) runs from the promise's continuation, outside the
// apply.
export function YieldPromise() {
  const a4 = action(function* () {
    yield Promise.resolve();
    setCount(4);
  });
  createEffect(() => 1, () => {
    a4();
  });
  return <div />;
}

// No row: a binding may hold a thenable.
export function YieldBinding() {
  const p = Promise.resolve();
  const a5 = action(function* () {
    yield p;
    setCount(5);
  });
  createEffect(() => 1, () => {
    a5();
  });
  return <div />;
}

// No row: from `onSettled` the flush throws its own error ("Cannot call flush() from inside onSettled ..."), not this one.
export function FromSettled() {
  const a6 = action(function* () {
    yield 1;
    setCount(6);
  });
  onSettled(() => {
    a6();
  });
  return <div />;
}

// No row: an async generator's `next()` returns a promise, so its next step runs from a continuation.
export function AsyncGenerator() {
  const a8 = action(async function* () {
    yield 1;
    setCount(8);
  });
  createEffect(() => 1, () => {
    a8();
  });
  return <div />;
}

// No row (a known miss): code before the `yield` may throw, and then no step resumes; nothing proves it does not.
export function CodeBeforeYield() {
  const a9 = action(function* () {
    setCount(9);
    yield 1;
  });
  createEffect(() => 1, () => {
    a9();
  });
  return <div />;
}

// ADR 1051's review: the operand must be one literal node. `yield` binds looser than `||`, `?:` and `+`, so each of these
// yields what the whole expression gives: a promise (assimilated, no flush on the apply's stack) or a throw (the step
// settles the action, no flush). No row.
function load(): Promise<void> {
  return Promise.resolve();
}
function fail(): string {
  throw new Error("fail");
}
const mode: string = "p";
export function YieldOrPromise() {
  const b1 = action(function* () {
    yield "z" < mode || load() || "";
    setCount(11);
  });
  createEffect(() => 1, () => {
    b1();
  });
  return <div />;
}
export function YieldConditionalPromise() {
  const b2 = action(function* () {
    yield "p" === mode ? load() : "done";
    setCount(12);
  });
  createEffect(() => 1, () => {
    b2();
  });
  return <div />;
}
export function YieldConcatThrows() {
  const b3 = action(function* () {
    yield "a" + fail() + "b";
    setCount(13);
  });
  createEffect(() => 1, () => {
    b3();
  });
  return <div />;
}

// The action calls the generator function before the first step: a parameter list that throws there ends the call, with
// no step and no flush. No row.
export function ParameterDestructured() {
  const b4 = action(function* ({ id }: { id: number } = undefined as any) {
    yield 1;
    setCount(id);
  });
  createEffect(() => 1, () => {
    b4();
  });
  return <div />;
}
export function ParameterDefaultThrows() {
  const b5 = action(function* (n: string = fail()) {
    yield 1;
    setCount(n.length);
  });
  createEffect(() => 1, () => {
    b5();
  });
  return <div />;
}
