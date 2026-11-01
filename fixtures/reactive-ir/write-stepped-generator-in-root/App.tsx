import { action, createOwner, createRoot, createSignal, runWithOwner, untrack } from "solid-js";

// Violations: the generator's one `next()` runs its body up to the first
// `yield` on the stack of the creating call, under the root that call is in;
// rc.17 throws REACTIVE_WRITE_IN_OWNED_SCOPE (ACTION_CALLED_IN_OWNED_SCOPE for
// the action) when the button is clicked.

// A write in a generator stepped in a root created in a handler.
export function HandlerRootSteppedWrite() {
  const [w1, setW1] = createSignal(0);
  return <div>{w1()}<button onClick={() => { createRoot(() => { (function* () { setW1(7); yield 1; })().next(); }); }}>go</button></div>;
}

// The same under a bound owner: uncertifiable, as every layer there is (the
// bound owner's callback is unplaced in a handler); no longer certified clean.
export function HandlerOwnerSteppedWrite() {
  const [w1, setW1] = createSignal(0);
  const boundOwner = createOwner();
  return <div>{w1()}<button onClick={() => { runWithOwner(boundOwner, () => { (function* () { setW1(7); yield 1; })().next(); }); }}>go</button></div>;
}

// An action call in a generator stepped in a root created in a handler.
export function HandlerRootSteppedAction() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(7); });
  return <div>{w1()}<button onClick={() => { createRoot(() => { (function* () { a1(); yield 1; })().next(); }); }}>go</button></div>;
}

// A write in a generator stepped in a root created in the body (uncertifiable before).
export function BodyRootSteppedWrite() {
  const [w1, setW1] = createSignal(0);
  createRoot(() => { (function* () { setW1(7); yield 1; })().next(); });
  return <div>{w1()}</div>;
}

// A `yield` in the other arm of an `if` cannot precede the write: the first step reaches it.
export function HandlerRootExclusiveArms(props: { flag: boolean }) {
  const [w1, setW1] = createSignal(0);
  return <div>{w1()}<button onClick={() => { createRoot(() => { (function* () { if (props.flag) { yield 1; } else { setW1(7); } })().next(); }); }}>go</button></div>;
}

// Legal (rc.17 silent).

// A generator stepped directly in the handler: no owner.
export function HandlerSteppedWrite() {
  const [w1, setW1] = createSignal(0);
  return <div>{w1()}<button onClick={() => { (function* () { setW1(7); yield 1; })().next(); }}>go</button></div>;
}

// Under `untrack` in the handler: the handler's owner, none.
export function HandlerUntrackSteppedWrite() {
  const [w1, setW1] = createSignal(0);
  return <div>{w1()}<button onClick={() => { untrack(() => { (function* () { setW1(7); yield 1; })().next(); }); }}>go</button></div>;
}

// Under `runWithOwner(null)` in the handler: no owner.
export function HandlerNullOwnerSteppedWrite() {
  const [w1, setW1] = createSignal(0);
  return <div>{w1()}<button onClick={() => { runWithOwner(null, () => { (function* () { setW1(7); yield 1; })().next(); }); }}>go</button></div>;
}

// A strict read in the root: the window is open under a root created in a handler.
export function HandlerRootSteppedRead() {
  const [w1, setW1] = createSignal(0);
  return <div>{w1()}<button onClick={() => { setW1(1); createRoot(() => { (function* () { w1(); yield 1; })().next(); }); }}>go</button></div>;
}

// The write follows a `yield` in the body's straight-line flow: the one step never reaches it.
export function HandlerRootWriteAfterYield() {
  const [w1, setW1] = createSignal(0);
  return <div>{w1()}<button onClick={() => { createRoot(() => { (function* () { yield 1; setW1(7); })().next(); }); }}>go</button></div>;
}

// `return()` on a fresh generator runs none of the body.
export function HandlerRootReturnStep() {
  const [w1, setW1] = createSignal(0);
  return <div>{w1()}<button onClick={() => { createRoot(() => { (function* () { setW1(7); yield 1; })().return(); }); }}>go</button></div>;
}

// An action body is a stepped generator the action owns (ADR 0520), not one stepped at its call: unchanged.
export function HandlerActionBody() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(7); });
  return <div>{w1()}<button onClick={() => { a1(); }}>go</button></div>;
}

// Uncertifiable: a `yield` in the loop that repeats the write may precede it,
// so the one step may stop first (it does: rc.17 silent).
export function HandlerRootLoopYield() {
  const [w1, setW1] = createSignal(0);
  return <div>{w1()}<button onClick={() => { createRoot(() => { (function* () { for (let i = 0; i < 2; i += 1) { if (i) { setW1(7); } yield 1; } })().next(); }); }}>go</button></div>;
}
