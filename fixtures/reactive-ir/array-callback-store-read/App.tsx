import { createMemo, createStore } from "solid-js";

// ADR 1181: the parameter of an inline callback the standard `map`/`forEach`/`filter`/`some`/`every`/`find` of a one-element
// array literal holding a store runs with is that store: `x.v` is the store's Get, as `s.v` written there.

// Violations: in the body window (rc.21 warns STRICT_READ_UNTRACKED).
export function MapInBody() {
  const [s] = createStore({ v: 0 });
  [s].map((x) => x.v);
  return <div />;
}

export function ForEachInBody() {
  const [s] = createStore({ v: 0 });
  [s].forEach((x) => {
    void x.v;
  });
  return <div />;
}

export function FindInBody() {
  const [s] = createStore({ v: 0 });
  [s].find((x) => x.v > 0);
  return <div />;
}

// Clean: in a memo's compute the Get tracks.
export function MapInMemo() {
  const [s] = createStore({ v: 0 });
  const m = createMemo(() => [s].map((x) => x.v)[0]);
  return <div>{m()}</div>;
}

// Uncertifiable or no row, as before: two elements, a written parameter, a non-literal receiver.
export function TwinTwoElements() {
  const [s] = createStore({ v: 0 });
  const [t] = createStore({ v: 1 });
  [s, t].map((x) => x.v);
  return <div />;
}

export function TwinWrittenParameter() {
  const [s] = createStore({ v: 0 });
  [s].map((x) => {
    x = { v: 2 };
    return x.v;
  });
  return <div />;
}

export function TwinListed() {
  const [s] = createStore({ v: 0 });
  const list = [s];
  list.map((x) => x.v);
  return <div />;
}
