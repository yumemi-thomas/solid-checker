import { createMemo, createSignal, createStore } from "solid-js";

// Each mention runs in an inline `forEach` callback before its destructuring
// declarator: rc.17 throws "Cannot access 'r' before initialization" there,
// and no Solid diagnostic. Uncertifiable, never a violation (ADR 0464).
export function EarlyRead() {
  [1].forEach(() => r());
  const [r] = createSignal(0);
  return <p>{r()}</p>;
}

export function EarlySetter() {
  [1].forEach(() => setR(1));
  const [r, setR] = createSignal(0);
  return <p>{r()}</p>;
}

export function EarlyStoreRead() {
  [1].forEach(() => s.v);
  const [s] = createStore({ v: 0 });
  return <p>{s.v}</p>;
}

export function EarlyStoreSetter() {
  [1].forEach(() => setS((draft) => { draft.v = 1; }));
  const [s, setS] = createStore({ v: 0 });
  return <p>{s.v}</p>;
}

export function EarlyStoreWrite() {
  [1].forEach(() => { s.v = 2; });
  const [s] = createStore({ v: 0 });
  return <p>{s.v}</p>;
}

// Through a hoisted nested declaration the early callback calls.
export function EarlyHelperRead() {
  [1].forEach(() => read());
  const [r] = createSignal(0);
  function read() { return r(); }
  return <p>{read()}</p>;
}

export function EarlyHelperWrite() {
  [1].forEach(() => write());
  const [r, setR] = createSignal(0);
  function write() { setR(1); }
  return <p>{r()}</p>;
}

// An identifier bound to a call result was already decided for call edges
// (ADR 0339); a site naming it early is now uncertifiable too.
export function EarlyMemo() {
  [1].forEach(() => m());
  const m = createMemo(() => 1);
  return <p>{m()}</p>;
}
