import { createProjection, createStore } from "solid-js";

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

// Negative: the callback named by the setter's argument runs in the window.
const byName = () => {
  store.n = 4;
};
export function CallbackByName() {
  return <button onClick={() => setStore(byName)}>{store.n}</button>;
}
// Negative: a function run through a method call inside the setter.
const viaMethod = () => {
  store.n = 5;
};
const api = { run() { viaMethod(); } };
export function MethodCaller() {
  return <button onClick={() => setStore(() => { api.run(); })}>{store.n}</button>;
}
// Negative: an exact const alias of the setter opens the same window.
const viaAlias = () => {
  store.n = 6;
};
const setAlias = setStore;
export function SetterAlias() {
  return <button onClick={() => { setStore(() => { viaAlias(); }); setAlias(() => { viaAlias(); }); }}>{store.n}</button>;
}
// Clean (ADR 0479): each helper has one call, so the chain climbs to the setter's window, where the read-proxy write lands.
function deep0() { store.n = 7; }
function deep1() { deep0(); }
function deep2() { deep1(); }
function deep3() { deep2(); }
export function DeepChain() {
  return <button onClick={() => setStore(() => { deep3(); })}>{store.n}</button>;
}
// Uncertifiable: an IIFE in JSX written inside the callback runs when the compiler renders it, after the setter returned.
export function JsxInWindow() {
  return <button onClick={() => setStore(() => { const v = <p>{(() => { store.n = 8; return 1; })()}</p>; void v; })}>{store.n}</button>;
}
// Uncertifiable: an async compute function closes its window at the first await.
const projected = createProjection<{ n: number }>(async () => {
  await Promise.resolve();
  projected.n = 1;
}, { n: 0 });
export function AsyncCompute() {
  return <p>{projected.n}</p>;
}
// Uncertifiable: the JSX the IIFE is in also holds another setter call's callback, not the window around this write.
export function JsxAroundOtherWindow() {
  return <button onClick={() => setStore(() => { const v = <div onClick={() => setStore(() => {})}>{(() => { store.n = 9; return 1; })()}</div>; void v; })}>{store.n}</button>;
}
// Negative: a write through an exact const alias of the store inside its own setter is a write through the store (legal.mjs found
// it reported; Chrome: it commits).
const storeAlias = store;
export function AliasInsideSetter() {
  return <button onClick={() => setStore(() => { storeAlias.n = 10; })}>{store.n}</button>;
}
