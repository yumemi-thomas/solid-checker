import { For, createContext, createMemo, createSignal, createStore, untrack, useContext } from "solid-js";

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

// ADR 1228: a store's member (or the store) reached through a context or a prop, handed as the receiver a standard function
// runs on (`call`, an at-once `bind`, `Reflect.apply`): a row at the call that runs it, where Chrome reports (ADR 1208's grid programs)
// STORE_WRITE_OUTSIDE_SETTER. Negative: another slot of the context's value (`ctx.other`), no row.
type Lists = { s1: { items: number[] }; other: number[] };
const ListCtx = createContext<Lists>();
function ContextSortCallChild() {
  const ctx = useContext<Lists>(ListCtx);
  Array.prototype.sort.call(ctx.s1.items, (x: number, y: number) => y - x);
  Array.prototype.sort.call(ctx.other, (x: number, y: number) => y - x);
  return <p>x</p>;
}
export function ContextSortCall() {
  const [s1] = createStore({ items: [1, 2] });
  return <ListCtx value={{ s1, other: [1, 2] }}><ContextSortCallChild /></ListCtx>;
}
function PropSortBindChild(props: { s1: { items: number[] } }) {
  return <button onClick={() => { Array.prototype.sort.bind(props.s1.items)((x: number, y: number) => y - x); }}>x</button>;
}
export function PropSortBind() {
  const [s1] = createStore({ items: [1, 2] });
  return <PropSortBindChild s1={s1} />;
}
function PropPushReflectChild(props: { list: number[] }) {
  return <button onClick={() => { Reflect.apply(Array.prototype.push, props.list, [1]); }}>x</button>;
}
export function PropPushReflect() {
  const [list] = createStore<number[]>([0]);
  return <PropPushReflectChild list={list} />;
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
