import { createStore } from "solid-js";

// ADR 1093: a store setter an expression-bodied hook returns in a literal (`() => ({ useIt: setStore })`), called through a member
// of the hook's result with a draft callback. The callback's first parameter is the setter's draft, so its write is the
// setter's own (ADR 0919). The setter reference in the returned literal now resolves through the binder when the Type Facts
// entity table holds no symbol for it.
export function Draft() {
  const [store, setStore] = createStore({ v: 0 });
  const useThing = () => ({ useIt: setStore });
  const thing = useThing();
  return <button onClick={() => { thing.useIt((s) => { s.v = 7; }); }}>{store.v}</button>;
}

// Kept: the member is a plain function that hands the store itself to the callback, not the setter (SC2003 uncertifiable).
export function Handed() {
  const [store] = createStore({ v: 0 });
  const useThing = () => ({ useIt: (f: (s: { v: number }) => void) => f(store) });
  const thing = useThing();
  return <button onClick={() => { thing.useIt((s) => { s.v = 7; }); }}>{store.v}</button>;
}

// Kept: a write through the store a hook's member holds, outside its setter (SC2003 violation, ADR 0497).
export function Misuse() {
  const [store] = createStore({ v: 0 });
  const useThing = () => ({ useIt: store });
  const thing = useThing();
  return <button onClick={() => { thing.useIt.v = 7; }}>{store.v}</button>;
}
