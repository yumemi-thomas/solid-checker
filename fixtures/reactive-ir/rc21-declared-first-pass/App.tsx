import { createEffect, createMemo, createTrackedEffect, untrack } from "solid-js";

// The creating pass reads the served value: the strict read warns, no throw.
export function PlainRead() {
  const memo = createMemo(async () => 1, { loadingValue: 5 });
  const one = memo();
  return <p>{one}</p>;
}

// Under `untrack` it is silent.
export function Untracked() {
  const memo = createMemo(async () => 1, { loadingValue: 5 });
  const one = untrack(() => memo());
  return <p>{one}</p>;
}

// A leaf read warns on a re-ask nothing here proves.
export function Leaf() {
  const memo = createMemo(async () => 1, { loadingValue: 5 });
  createTrackedEffect(() => {
    memo();
  });
  return <p>{1}</p>;
}

// Without a declared value it throws.
export function Undeclared() {
  const memo = createMemo(async () => 1);
  const one = memo();
  return <p>{one}</p>;
}

// Layers run on the creating pass's stack, and carry the same fact.
export function Layers() {
  const memo = createMemo(async () => 1, { loadingValue: 5 });
  const wrapUse = () => {
    memo();
  };
  wrapUse();
  (() => {
    memo();
  })();
  untrack(() => wrapUse());
  return <p>{1}</p>;
}

// An apply runs later: a re-ask throws there. Open.
export function Later() {
  const memo = createMemo(async () => 1, { loadingValue: 5 });
  const read = () => {
    memo();
  };
  createEffect(
    () => 1,
    () => {
      read();
    },
  );
  return <p>{1}</p>;
}

// Without a declared value a wrapper's read throws.
export function UndeclaredLayer() {
  const memo = createMemo(async () => 1);
  const wrapUse = () => {
    memo();
  };
  wrapUse();
  return <p>{1}</p>;
}
