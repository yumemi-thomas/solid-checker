import { createStore } from "solid-js";

// P1: a call after a statement that throws, in a `try` whose `catch` swallows: f never runs.
function guarded(f: () => void) {
  try {
    JSON.parse("x");
    f();
  } catch {}
}
export function TryThrowsDestructure() {
  const [st] = createStore({ v: 0 });
  guarded(() => {
    const { v } = st;
    void v;
  });
  return <i />;
}
export function TryThrowsMember() {
  const [st] = createStore({ v: 0 });
  guarded(() => {
    void st.v;
  });
  return <i />;
}
// Twin: the call comes first in the `try`, so it runs.
function first(f: () => void) {
  try {
    f();
    JSON.parse("x");
  } catch {}
}
export function TryCallFirst() {
  const [st] = createStore({ v: 0 });
  first(() => {
    void st.v;
  });
  return <i />;
}
// Twin: no `try`, the helper always calls f.
function plain(f: () => void) {
  JSON.stringify(1);
  f();
}
export function PlainHelper() {
  const [st] = createStore({ v: 0 });
  plain(() => {
    void st.v;
  });
  return <i />;
}

// P2: a standard inline callback that runs zero times.
export function NamedEmptyForEach() {
  const [st] = createStore({ v: 0 });
  const xs: number[] = [];
  xs.forEach(() => {
    const { v } = st;
    void v;
  });
  return <i />;
}
export function OneElementSort() {
  const [st] = createStore({ v: 0 });
  [1].sort(() => {
    const { v } = st;
    return v;
  });
  return <i />;
}
export function OneElementSortMember() {
  const [st] = createStore({ v: 0 });
  [1].sort(() => st.v);
  return <i />;
}
// Twins: two elements sort with the comparator; a filled name runs its callback.
export function TwoElementSort() {
  const [st] = createStore({ v: 0 });
  [1, 2].sort(() => st.v);
  return <i />;
}
export function NamedFilledForEach() {
  const [st] = createStore({ v: 0 });
  const xs: number[] = [];
  xs.push(1);
  xs.forEach(() => {
    void st.v;
  });
  return <i />;
}

// F2: a store handed through a standard inline callback's parameter.
export function MapParamDestructure() {
  const [st] = createStore({ v: 0 });
  [st].map(({ v }) => v);
  return <i />;
}
export function MapParamMember() {
  const [st] = createStore({ v: 0 });
  [st].map((s) => s.v);
  return <i />;
}
// Twin: the parameter holds a plain number; nothing is read.
export function MapParamPlain() {
  [1].map((s) => s + 1);
  return <i />;
}

// Twins of P1 the fix keeps: a catch that only rethrows completes no call it skipped.
function rethrown(f: () => void) {
  try {
    JSON.parse("1");
    f();
  } catch (e) {
    throw e;
  }
}
export function TryRethrowMember() {
  const [st] = createStore({ v: 0 });
  rethrown(() => {
    void st.v;
  });
  return <i />;
}
// A `finally` with no jump keeps the throw a throw.
function finished(f: () => void) {
  try {
    JSON.parse("1");
    f();
  } finally {
    void 0;
  }
}
export function TryFinallyMember() {
  const [st] = createStore({ v: 0 });
  finished(() => {
    void st.v;
  });
  return <i />;
}
// P2 twin: the name escapes (handed on), so it may be filled.
export function NamedEmptyHanded() {
  const [st] = createStore({ v: 0 });
  const xs: number[] = [];
  fill(xs);
  xs.forEach(() => {
    void st.v;
  });
  return <i />;
}
function fill(xs: number[]) {
  xs.push(1);
}
// The reachability reading: a call under a condition nothing decides.
function maybe(f: () => void) {
  if (Math.random() > 2) f();
}
export function BranchDestructure() {
  const [st] = createStore({ v: 0 });
  maybe(() => {
    const { v } = st;
    void v;
  });
  return <i />;
}
export function BranchMember() {
  const [st] = createStore({ v: 0 });
  maybe(() => {
    void st.v;
  });
  return <i />;
}
