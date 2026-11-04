import { createEffect, createStore } from "solid-js";

// A receiver holding exactly a `const` is that `const`.
export function Alias() {
  const [s] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: s });
  const thing = useThing();
  createEffect(async () => {
    await Promise.resolve();
    const x = thing.useIt.a; // SC1002
    return 1;
  }, () => {});
  return <p>x</p>;
}

export function AliasTracked() {
  const [s] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: s });
  const thing = useThing();
  createEffect(async () => {
    s.a;
    await Promise.resolve();
    const x = thing.useIt.a; // clean: the same key, read before the await
    return 1;
  }, () => {});
  return <p>x</p>;
}

// A function written before the await and run past it reads on the continuation.
export function Wrapper() {
  const [s] = createStore({ a: { v: 0 } });
  const wrapUse = () => {
    const x = s.a; // SC1002
  };
  createEffect(async () => {
    await Promise.resolve();
    wrapUse();
    return 1;
  }, () => {});
  return <p>x</p>;
}

export function WrapperInClosure() {
  const [s] = createStore({ a: { v: 0 } });
  const wrapUse = () => {
    const x = s.a; // SC1002
  };
  createEffect(async () => {
    await Promise.resolve();
    const f = () => {
      wrapUse();
    };
    f();
    return 1;
  }, () => {});
  return <p>x</p>;
}

export function WrapperTracked() {
  const [s] = createStore({ a: { v: 0 } });
  const wrapUse = () => {
    const x = s.a; // clean: the same key, read before the await
  };
  createEffect(async () => {
    s.a;
    await Promise.resolve();
    wrapUse(); // uncertifiable at the call, as before ADR 0614
    return 1;
  }, () => {});
  return <p>x</p>;
}

export function WrapperBefore() {
  const [s] = createStore({ a: { v: 0 } });
  const wrapUse = () => {
    const x = s.a; // clean: run before the await, a dependency
  };
  createEffect(async () => {
    wrapUse();
    await Promise.resolve();
    return 1;
  }, () => {});
  return <p>x</p>;
}
