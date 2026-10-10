import { createSignal } from "solid-js";

// Keeps the callback and runs it from a timer, never during the call.
const pending: (() => void)[] = [];
export function later(callback: () => void) {
  pending.push(callback);
}
setInterval(() => {
  for (const callback of pending.splice(0)) callback();
}, 100);

// Runs the source during the call and keeps the closure it returns.
let kept: (() => void) | undefined;
export function keep(source: () => () => void) {
  kept = source();
}

// Runs the callback during the call.
export function now(callback: () => void) {
  callback();
}

// Clean: `later` stores the callback, which runs from the interval.
export function DeferredByCallee() {
  const [count, setCount] = createSignal(0);
  later(() => setCount(1));
  return <p>{count()}</p>;
}

// Clean: the write is in the closure `keep`'s source returns, which runs
// only from the click handler.
export function ReturnedClosure() {
  const [count, setCount] = createSignal(0);
  keep(() => () => setCount(1));
  return <button onClick={() => kept?.()}>{count()}</button>;
}

// Clean: `@solid-primitives/history`'s `createUndoHistory` shape. The source
// reads during the call and returns the setter closure, which the history runs
// later from `undo()`.
export function UndoHistoryShape() {
  const [count, setCount] = createSignal(0);
  keep(() => {
    const value = count();
    return () => setCount(value);
  });
  return <button onClick={() => kept?.()}>{count()}</button>;
}

// Clean: a stored closure whose only call is in the click handler.
export function StoredClosure() {
  const [count, setCount] = createSignal(0);
  const reset = () => setCount(0);
  return <button onClick={() => reset()}>{count()}</button>;
}

// Violation: `now` invokes its parameter during the call, in the body.
export function ProvenSynchronous() {
  const [count, setCount] = createSignal(0);
  now(() => setCount(1));
  return <p>{count()}</p>;
}

// Violation: the stored closure is called in the body.
export function StoredAndCalled() {
  const [count, setCount] = createSignal(0);
  const reset = () => setCount(0);
  reset();
  return <p>{count()}</p>;
}

// Violation: a standard-library inline callback runs during the call.
export function InlineCallback() {
  const [count, setCount] = createSignal(0);
  [1].forEach(() => setCount(1));
  return <p>{count()}</p>;
}

// Violation: an IIFE runs where it is written.
export function ImmediatelyInvoked() {
  const [count, setCount] = createSignal(0);
  (() => setCount(1))();
  return <p>{count()}</p>;
}
