import { createEffect, createMemo, createSignal } from "solid-js";

// ---- A read in the body of a `for await` that provably runs it, in an async computation (ADR 1164): every run follows the loop's await ----

// Violation: the body runs once ([1]'s first `next()` fulfills), after the loop awaited it, in the memo's flight.
export function ForAwaitRead() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      s();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Violation: a branch in the body, like any branch past an await.
export function ForAwaitReadGuarded() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      if (_x > 0) s();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}

// ---- Not proven: uncertifiable as before ----

// The body never runs: a provably dead site is never a violation (ADR 1018).
export function ForAwaitEmpty() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of []) {
      s();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// An iterable that is not a literal: whether the body runs is not read.
export function ForAwaitNamed() {
  const [s] = createSignal(0);
  const xs = [1];
  const m = createMemo(async () => {
    for await (const _x of xs) {
      s();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// A spread first: the literal may be empty.
export function ForAwaitSpread() {
  const [s] = createSignal(0);
  const xs: number[] = [];
  const m = createMemo(async () => {
    for await (const _x of [...xs, 1]) {
      s();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// A promise element: the first `next()` adopts it, and a rejected one never runs the body (this one fulfills; the proof does not read it).
export function ForAwaitPromiseElement() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of [Promise.resolve(1)]) {
      s();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// The source was read before the first suspension: a dependency, not warned about (Chrome quiet).
export function ForAwaitTracked() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    s();
    for await (const _x of [1]) {
      s();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Violation since ADR 1191: an IIFE the body runs in place reads on the same continuation.
export function ForAwaitClosure() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      (() => s())();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// After the loop: not the body.
export function ForAwaitAfterLoop() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    for await (const _x of [1]) {
      void _x;
    }
    s();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}

// ---- A `finally` after `try { await Promise.resolve(); }` runs only past the await (ADR 1164): a write there has no owner ----

// Clean: the global `Promise.resolve()` cannot throw before the await suspends.
export function FinallyWrite() {
  const [, setW] = createSignal(0);
  const m = createMemo(async () => {
    try { await Promise.resolve(); } finally {
      setW(1);
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Clean: a literal argument, and the `catch` too.
export function FinallyWriteLiteral() {
  const [, setW] = createSignal(0);
  const m = createMemo(async () => {
    try { await Promise.resolve(1); } catch {
      setW(2);
    } finally {
      setW(3);
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Violation (unchanged): the read in the `finally` is in the memo's flight.
export function FinallyRead() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    try { await Promise.resolve(); } finally {
      s();
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}

// ---- Not proven: uncertifiable as before ----

// A name argument may hold an object whose adoption runs code.
export function FinallyWriteNamedArgument() {
  const [, setW] = createSignal(0);
  const v = 1;
  const m = createMemo(async () => {
    try { await Promise.resolve(v); } finally {
      setW(4);
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// A local `Promise` shadows the global.
export function FinallyWriteShadowed() {
  const [, setW] = createSignal(0);
  const Promise = { resolve: (): number => 1 };
  const m = createMemo(async () => {
    try { await Promise.resolve(); } finally {
      setW(5);
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Not the identifier `Promise`: a member of `globalThis`.
export function FinallyWriteMember() {
  const [, setW] = createSignal(0);
  const m = createMemo(async () => {
    try { await globalThis.Promise.resolve(); } finally {
      setW(6);
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
// Code before the await in the `try` may throw first: the write at its head is in the owned scope (violation), the `finally` unplaced.
export function FinallyWriteAfterCode() {
  const [, setW] = createSignal(0);
  const m = createMemo(async () => {
    try { setW(7); await Promise.resolve(); } finally {
      setW(8);
    }
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
