import { createEffect, createMemo, createStore, onCleanup, resolve, until } from "solid-js";

declare function ready(): boolean;
declare function name(): string;
async function save(_value: string): Promise<void> {}

// SC2004, SC2005: the guard reads the observer, and no task past an await has one.
export function Observer() {
  const m = createMemo(async () => {
    await Promise.resolve();
    resolve(() => 1); // clean
    (() => {
      until(() => 1); // clean, through an IIFE
    })();
    return 1;
  });
  const n = createMemo(async () => {
    if (ready()) await Promise.resolve();
    resolve(() => 2); // uncertifiable: the await is in a branch
    return 1;
  });
  const o = createMemo(async () => {
    resolve(() => 3); // violation: before the await, under the observer
    await Promise.resolve();
    return 1;
  });
  createEffect(() => [m(), n(), o()], () => {});
  return <div />;
}

// SC4001: a compute's region owns its synchronous run only.
export function Owner() {
  const m = createMemo(async () => {
    createEffect(() => 1, () => {}); // owned: before the await
    await Promise.resolve();
    createEffect(() => 2, () => {}); // violation
    onCleanup(() => {}); // violation
    return 1;
  });
  const n = createMemo(async () => {
    try {
      await save(name());
    } finally {
      onCleanup(() => {}); // uncertifiable: `name()` may throw before the await
    }
    return 1;
  });
  createEffect(() => [m(), n()], () => {});
  return <div />;
}

// SC1002: the object of a written member is read past the await; a link read before it is a dependency already.
export function Link() {
  const [s] = createStore({ a: { v: 0 } });
  const [t] = createStore({ a: { v: 0 } });
  const m = createMemo(async () => {
    const seen = t.a;
    await Promise.resolve();
    s.a.v = 1; // SC1002 for `s.a`, SC2003 for the write
    t.a.v = 2; // SC2003 only: `t.a` is tracked
    return seen;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// SC1002 through a function run in place past the await; a key read before it stays a dependency.
export function LayeredRead() {
  const [s] = createStore({ a: { v: 0 } });
  const [t] = createStore({ a: { v: 0 } });
  const m = createMemo(async () => {
    const seen = t.a.v;
    await Promise.resolve();
    const read = () => {
      const late = s.a.v; // violation
      const known = t.a.v; // tracked before the await
      return late + known;
    };
    return read() + seen;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// A hoisted function called before the await runs on the compute's first stack, whatever its text position.
export function Hoisted() {
  const [s] = createStore({ a: { v: 0 } });
  const m = createMemo(async () => {
    helper();
    await Promise.resolve();
    function helper() {
      return s.a.v; // tracked: no SC1002
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
