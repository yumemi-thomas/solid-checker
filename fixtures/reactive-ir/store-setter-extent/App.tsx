import { createStore } from "solid-js";

declare function later(callback: () => void): void;

const [store, setStore] = createStore({ n: 0, box: Object.freeze({ inner: { n: 0 } }), map: new Map<string, { n: number }>() });

function runNow(callback: () => void) {
  callback();
}

// Positive: a write the setter's callback schedules runs after the setter returned, with the draft closed: dropped.
export function DeferredMicrotask() {
  return <button onClick={() => setStore(() => { queueMicrotask(() => { store.n = 1; }); })}>{store.n}</button>;
}
export function DeferredTimeout() {
  return <button onClick={() => setStore(() => { setTimeout(() => { store.n = 1; }, 0); })}>{store.n}</button>;
}
export function DeferredThen() {
  return <button onClick={() => setStore(() => { Promise.resolve().then(() => { store.n = 1; }); })}>{store.n}</button>;
}
// Positive: an await inside an IIFE the callback runs: the write follows it, after the setter returned.
export function AfterAwait() {
  return <button onClick={() => setStore(() => { void (async () => { await Promise.resolve(); store.n = 1; })(); })}>{store.n}</button>;
}

// Negative: the write runs on the callback's own stack.
export function Direct() {
  return <button onClick={() => setStore(() => { store.n = 1; })}>{store.n}</button>;
}
export function InIife() {
  return <button onClick={() => setStore(() => { (() => { store.n = 1; })(); })}>{store.n}</button>;
}
export function InForEach() {
  return <button onClick={() => setStore(() => { [1].forEach(() => { store.n = 1; }); })}>{store.n}</button>;
}
export function InHelper() {
  return <button onClick={() => setStore(() => { runNow(() => { store.n = 1; }); })}>{store.n}</button>;
}

// Negative: a function written outside the setter, every call of which runs inside it (docs/open-work.md's former false positive).
const bump = () => {
  store.n = 1;
};
export function CalledInside() {
  return <button onClick={() => setStore(() => { bump(); })}>{store.n}</button>;
}

// Positive: a function called inside the setter and outside it: the outside run drops the write.
const bumpBoth = () => {
  store.n = 2;
};
export function CalledBoth() {
  return (
    <button onClick={() => { setStore(() => { bumpBoth(); }); bumpBoth(); }}>{store.n}</button>
  );
}

// Uncertifiable: a callback handed to a function this does not know, inside the setter.
export function UnknownCallee() {
  return <button onClick={() => setStore(() => { later(() => { store.n = 1; }); })}>{store.n}</button>;
}
// Uncertifiable: a function called inside the setter that is also handed on.
const bumpHanded = () => {
  store.n = 3;
};
export function CalledAndHanded() {
  return <button onClick={() => { setStore(() => { bumpHanded(); }); later(bumpHanded); }}>{store.n}</button>;
}

// Uncertifiable: a write past the root, where the link may be a value the store does not wrap (frozen, a Map's entry).
export function PastFrozenLink() {
  return <button onClick={() => { store.box.inner.n = 1; }}>{store.n}</button>;
}
export function PastMapEntry() {
  return <button onClick={() => { store.map.get("k")!.n = 1; }}>{store.n}</button>;
}
// Positive: the root's own property, wrapped whatever the initial value is.
export function RootWrite() {
  return <button onClick={() => { store.n = 1; }}>{store.n}</button>;
}
