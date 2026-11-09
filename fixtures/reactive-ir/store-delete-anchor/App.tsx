import { createStore } from "solid-js";

type State = { a: { b?: number }; r: { b: number } };

const [store, setStore] = createStore<State>({ a: { b: 1 }, r: { b: 1 } });
const plain: State = { a: { b: 1 }, r: { b: 1 } };
const id = <T,>(x: T): T => x;

// ADR 0922: the compiler drops a delete operand's parentheses, `as`,
// `satisfies` and `!`, and V8 places the delete at the last positioned
// expression the operand evaluates (here the call `id`, or the link `a`).
// Each row starts at the chain's root through the wrappers.

// Positive, at `id`: a cast call result (the fuzz shape sw-delete-call),
// and its plain twin, which TypeScript accepts without the cast.
export function CastCall() {
  return <button onClick={() => { delete (id(store.r) as { b?: number }).b; }}>go</button>;
}
export function PlainCall() {
  return <button onClick={() => { delete id(store.a).b; }}>go</button>;
}

// Positive, at `id`: the same call under `satisfies` and under `!`.
export function SatisfiesCall() {
  return <button onClick={() => { delete (id(store.a) satisfies { b?: number }).b; }}>go</button>;
}
export function NonNullCall() {
  return <button onClick={() => { delete id(store.a)!.b; }}>go</button>;
}

// Positive, at `store`: a cast link of the store itself, and its plain twin.
export function CastLink() {
  return <button onClick={() => { delete (store.r as { b?: number }).b; }}>go</button>;
}
export function PlainLink() {
  return <button onClick={() => { delete store.a.b; }}>go</button>;
}

// Negative: the same cast delete on a plain object, and inside the store's
// own setter, which write-enables its proxy.
export function PlainObject() {
  return <button onClick={() => { delete (id(plain.r) as { b?: number }).b; }}>go</button>;
}
export function OwnSetter() {
  return <button onClick={() => setStore(() => { delete (id(store.r) as { b?: number }).b; })}>go</button>;
}

// An assignment through the same cast keeps the member's start: its frame is
// the written property, which the judge reads as the member's start.
export function CastAssign() {
  return <button onClick={() => { (id(store.r) as { b?: number }).b = 2; }}>go</button>;
}

// The same deletes in a component body, where the write runs as it renders.
export function BodyCastCall() {
  delete (id(store.r) as { b?: number }).b;
}
export function BodyPlainCall() {
  delete id(store.a).b;
}
export function BodyCastLink() {
  delete (store.r as { b?: number }).b;
}
export function BodyCastRoot() {
  delete (store as { a: { b?: number } }).a.b;
}
export function BodyPlainLink() {
  delete store.a.b;
}
