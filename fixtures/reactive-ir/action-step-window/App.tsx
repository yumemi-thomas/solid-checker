import { action, createEffect, createReaction, createSignal, createTrackedEffect, onSettled } from "solid-js";

// An effect apply calls the action: the first slice reads in the apply's window.
export function FromApply() {
  const [r1] = createSignal(0);
  const act = action(function* () {
    r1();
  });
  createEffect(() => 1, () => {
    act();
  });
  return <div />;
}

// A plain-value `yield` resumes on the same stack: still the apply's window. A
// yielded promise resumes from a microtask: no window.
export function PlainYield() {
  const [r1] = createSignal(0);
  const [r2] = createSignal(0);
  const act = action(function* () {
    yield 1;
    r1();
    yield Promise.resolve();
    r2();
  });
  createEffect(() => 1, () => {
    act();
  });
  return <div />;
}

// A reaction callback is an apply too.
export function FromReaction() {
  const [r1] = createSignal(0);
  const [t1, setT1] = createSignal(1);
  const act = action(function* () {
    r1();
  });
  const track = createReaction(() => {
    act();
  });
  track(() => t1());
  return <button onClick={() => setT1(2)}>go</button>;
}

// A handler, an error arm, an owned onSettled and a tracked effect run the
// slice in no window: legal.
export function SilentCallers() {
  const [r1] = createSignal(0);
  const [t1, setT1] = createSignal(1);
  const fromHandler = action(function* () {
    r1();
  });
  const fromError = action(function* () {
    r1();
  });
  const fromLeaf = action(function* () {
    r1();
  });
  const never = action(function* () {
    r1();
  });
  createEffect(() => { if (t1() === 2) throw new Error("x"); return 1; }, { effect: () => {}, error: () => { fromError(); } });
  onSettled(() => {
    fromLeaf();
  });
  createTrackedEffect(() => {
    fromLeaf();
  });
  return <button onClick={() => { setT1(2); fromHandler(); }}>go</button>;
}

// The handler is the action itself: the host dispatches it in no window.
export function HandlerValue() {
  const [r1] = createSignal(0);
  const act = action(function* () {
    r1();
  });
  return <button onClick={act}>go</button>;
}

// The apply and a handler both call it: the apply's run warns, but the slice's
// run is proven only where every call's is: an obligation.
export function MixedCallers() {
  const [r1] = createSignal(0);
  const act = action(function* () {
    r1();
  });
  createEffect(() => 1, () => {
    act();
  });
  return <button onClick={() => act()}>go</button>;
}

// The binding escapes to a callee nothing follows: unplaced.
export function Escaped() {
  const [r1] = createSignal(0);
  const act = action(function* () {
    r1();
  });
  const box = { act };
  createEffect(() => 1, () => {
    box.act();
  });
  return <div />;
}
