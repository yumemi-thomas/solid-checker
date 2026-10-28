import { createMemo, createSignal, createStore } from "solid-js";
import { shared, setShared } from "./state";

// Positive controls: the same mentions after the declarator stay violations.
export function LateSignal() {
  const [r, setR] = createSignal(0);
  [1].forEach(() => r());
  [1].forEach(() => setR(1));
  return <p>{r()}</p>;
}

export function LateStore() {
  const [s, setS] = createStore({ v: 0 });
  [1].forEach(() => s.v);
  [1].forEach(() => setS((draft) => { draft.v = 1; }));
  return <p>{s.v}</p>;
}

export function LateStoreWrite() {
  const [s] = createStore({ v: 0 });
  [1].forEach(() => { s.v = 2; });
  return <p>{s.v}</p>;
}

// A hoisted nested declaration every call of which follows the declarator.
export function LateHelper() {
  const [r] = createSignal(0);
  [1].forEach(() => read());
  function read() { return r(); }
  return <p>{read()}</p>;
}

export function LateMemo() {
  const m = createMemo(() => 1);
  [1].forEach(() => m());
  return <p>{m()}</p>;
}

// Across a one-way import the declaring module has run first.
export function Imported() {
  shared();
  setShared(1);
  return <p>{shared()}</p>;
}
