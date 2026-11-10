import { For, createMemo, createSignal, createStore, untrack } from "solid-js";

// Positive: what a call returns, called again in the body (row at the outer call).
export function FnAlias() {
  const [count] = createSignal(0);
  const f = () => count;
  const g = f;
  void g()();
  return <p>x</p>;
}

export function SlotReturn() {
  const [count] = createSignal(0);
  const v = { f: () => count };
  void v.f()();
  return <p>x</p>;
}

export function ClassReturn() {
  const [count] = createSignal(0);
  class K { f() { return count; } }
  void new K().f()();
  return <p>x</p>;
}

// Positive: the element is the call (row at `<`, not at the tag's name).
export function Tag() {
  const [count] = createSignal(0);
  const H = count as unknown as () => null;
  void (<H />);
  return <p>x</p>;
}

// Positive: a spread hands its elements to the callee's parameters.
export function Spread() {
  const [count] = createSignal(0);
  ((h: () => number) => void h())(...([count] as [() => number]));
  return <p>x</p>;
}

// Positive: `For` holds its child; a call through its element runs it.
export function ChildFor() {
  const [count] = createSignal(0);
  const el = <For each={[1]}>{count}</For>;
  void (el as unknown as () => unknown)();
  return <p>x</p>;
}

// Positive: a created memo carried by `Object.values`, called through casts
// (row at the callee's start, not the parenthesis).
export function CreatedValues() {
  void (Object.values({ a: createMemo(() => 1) })[0] as () => number)();
  return <p>x</p>;
}

// Wrapper: a labelled `untrack` reads at its own call, inside a helper.
export function UntrackLabelWrapper() {
  const [count] = createSignal(0);
  const wrap = () => { void untrack(count, "x"); };
  wrap();
  return <p>x</p>;
}

// Positive: a bound `push` of a store root writes it in a handler.
export function StorePushHeld() {
  const [list] = createStore<number[]>([0]);
  return <button onClick={() => { const f = Array.prototype.push.bind(list); f(1); }}>x</button>;
}

// Negative: the same calls in a memo's compute are tracked: no row at the call.
export function TrackedTwins() {
  const [count] = createSignal(0);
  const f = () => count;
  const g = f;
  const v = { f: () => count };
  const m = createMemo(() => g()() + v.f()() + ((h: () => number) => h())(...([count] as [() => number])));
  return <p>{m()}</p>;
}
