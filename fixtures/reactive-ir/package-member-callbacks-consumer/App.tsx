import { createSignal } from "solid-js";
import { callBoundOnly, callBoundOnlyOpen, callHandler } from "reactive-package";

const [count] = createSignal(0);
const event = { defaultPrevented: false };

function readCount(_first?: unknown, _second?: unknown) {
  count();
}

// The reference: calling `readCount` in the component body is an untracked
// read of `count`.
export function Direct() {
  readCount();
  return <div />;
}

// `callHandler`'s accepted contract closes `callbacks` over a call of argument
// 1 (`handler(event)`) and a call of its member 0 (`handler[0](handler[1],
// event)`), beside reads of both arguments. Each shape folds `readCount` as the
// inline call it is: the function itself through the first item, and the
// pair's function through the member item, whose path the call's own array
// literal resolves.
export function Handler() {
  callHandler(event, readCount);
  return <div />;
}

export function HandlerPair() {
  callHandler(event, [readCount, 1]);
  return <div />;
}

// `callBoundOnly` calls member 0 of argument 1 and never the argument itself.
// The pair's function is folded as an inline call; a function passed whole is
// not called, and is not folded as if it were.
export function BoundPair() {
  callBoundOnly(event, [readCount, 1]);
  return <div />;
}

export function BoundWhole() {
  callBoundOnly(event, readCount);
  return <div />;
}

// An argument whose member the call's syntax does not show is not folded at
// all, although this one holds `readCount` at 0: the member called is
// whatever the caller's value holds there, and following a binding to the
// literal that made it is not a fact this consumer states.
function pair(): [typeof readCount, number] {
  return [readCount, 1];
}
const stored = pair();

export function BoundStored() {
  callBoundOnly(event, stored);
  return <div />;
}

// The control for `BoundWhole`: the same call with `callbacks` left open
// reports the open claim at the import, because the package may call the
// function it was handed.
export function BoundOpen() {
  callBoundOnlyOpen(event, readCount);
  return <div />;
}
