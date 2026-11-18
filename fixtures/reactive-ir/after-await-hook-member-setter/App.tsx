import { createMemo, createSignal, createStore } from "solid-js";

// ADR 1098: a signal setter a hook's returned literal carries, called after an await (`thing.useIt(7)` in a promise handler). The
// setter is the table's (the transport the write rules use), declared by an exact `const [get, set] = createSignal(…)` with no
// `equals` function, and its argument is inert: the call runs no project code, as the plain setter call's does (ADR 0976).
export function Carried() {
  const [value, setValue] = createSignal(0);
  const useThing = () => ({ useIt: setValue });
  const thing = useThing();
  Promise.resolve().then(() => {
    thing.useIt(7);
  });
  return <p>{value()}</p>;
}

// Kept: an `equals` function the setter runs is project code.
export function Compared() {
  const [value, setValue] = createSignal(0, { equals: (a, b) => a === b });
  const useThing = () => ({ useIt: setValue });
  const thing = useThing();
  Promise.resolve().then(() => {
    thing.useIt(7);
  });
  return <p>{value()}</p>;
}

// Proved (ADR 1098 with ADR 1089 D): a store's setter carried by the member, given a function that only writes a literal under a
// static key of its parameter, runs nothing else of the project's.
export function StoreDraft() {
  const [store, setStore] = createStore({ v: 0 });
  const useThing = () => ({ useIt: setStore });
  const thing = useThing();
  Promise.resolve().then(() => {
    thing.useIt((s) => { s.v = 7; });
  });
  return <p>{store.v}</p>;
}

// Kept: a store's setter given a value, not a function (ADR 1089 D models only the updater form).
export function StoreCarried() {
  const [store, setStore] = createStore({ v: 0 });
  const useThing = () => ({ useIt: setStore });
  const thing = useThing();
  Promise.resolve().then(() => {
    thing.useIt({ v: 7 });
  });
  return <p>{store.v}</p>;
}

// Kept: an async memo's read of a carried getter after its own await is the walk's violation.
export function AsyncRead() {
  const [value] = createSignal(0);
  const useThing = () => ({ useIt: value });
  const thing = useThing();
  const shown = createMemo(async () => {
    await Promise.resolve();
    return thing.useIt();
  });
  return <p>{shown()}</p>;
}
