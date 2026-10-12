import { createStore } from "solid-js";

// Solid 2 write-enables a store's original proxy for the duration of its own
// setter's draft callback. Transparent TypeScript wrappers preserve the
// setter's identity, so neither wrapped call below is a dropped write.
export function AssertedOwnSetter() {
  const [store, setStore] = createStore({ open: false });
  return <button onClick={() => (setStore as typeof setStore)(() => { store.open = true; })}>{String(store.open)}</button>;
}

export function NonNullOwnSetter() {
  const [store, setStore] = createStore({ open: false });
  return <button onClick={() => setStore!(() => { store.open = true; })}>{String(store.open)}</button>;
}

// Positive: another store's setter enables nothing for this proxy, wrapped or
// not, so the write is dropped (SC2003).
export function AssertedOtherSetter() {
  const [store] = createStore({ open: false });
  const [other, setOther] = createStore({ count: 0 });
  return <button onClick={() => (setOther as typeof setOther)(() => { store.open = true; })}>{String(store.open)}{other.count}</button>;
}

// Nested wrappers peel to the same identifier.
export function NestedOwnSetter() {
  const [store, setStore] = createStore({ open: false });
  return <button onClick={() => ((setStore as typeof setStore)!)(() => { store.open = true; })}>{String(store.open)}</button>;
}

export function SatisfiedOwnSetter() {
  const [store, setStore] = createStore({ open: false });
  return <button onClick={() => ((setStore satisfies typeof setStore)!)(() => { store.open = true; })}>{String(store.open)}</button>;
}
