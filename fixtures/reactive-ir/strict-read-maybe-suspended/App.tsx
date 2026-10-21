import { createSignal, createStore } from "solid-js";

// ADR 0354: a read is placed in the strict-read window only when no await of
// its async function may run before it. An await in the other arm of the same
// `if` or `?:` does not run before it, and an unconditional await (ADR 0323)
// ends the window for good. Everything else that may run after an await is
// uncertifiable, never a violation.

function now(f: () => void) {
  f();
}

declare const c: boolean;
declare const p: Promise<number>;
declare const q: Promise<{ a?: number }>;
declare const ys: AsyncIterable<number>;

// Violation: the read runs before any await of its function.
export function ReadBeforeAwait() {
  const [count] = createSignal(0);
  now(async () => {
    console.log(count());
    await p;
  });
  return <div />;
}

// Uncertifiable: a conditional await may precede the read.
export function IfAwaitRead() {
  const [count] = createSignal(0);
  now(async () => {
    if (c) await p;
    console.log(count());
  });
  return <div />;
}

// Violation: the read is in the other arm of the `if`, which runs only when
// the await does not.
export function IfAwaitElseRead() {
  const [count] = createSignal(0);
  now(async () => {
    if (c) {
      await p;
    } else {
      console.log(count());
    }
  });
  return <div />;
}

// Violation: the read is in the alternate of a `?:` whose consequent awaits.
export function ConditionalAwaitAlternateRead() {
  const [count] = createSignal(0);
  now(async () => {
    c ? await p : console.log(count());
  });
  return <div />;
}

// Violation: the read is in the consequent of a `?:` whose alternate awaits,
// so it runs before any await.
export function ConditionalAwaitConsequentRead() {
  const [count] = createSignal(0);
  now(async () => {
    c ? console.log(count()) : await p;
  });
  return <div />;
}

// Uncertifiable: a loop with an await runs before the read after it.
export function LoopAwaitRead() {
  const [count] = createSignal(0);
  now(async () => {
    for (const x of [1]) {
      await p;
      void x;
    }
    console.log(count());
  });
  return <div />;
}

// Uncertifiable: a later iteration reads after the await of an earlier one.
export function LoopBackEdgeRead() {
  const [count] = createSignal(0);
  now(async () => {
    for (const x of [1]) {
      console.log(count());
      await p;
      void x;
    }
  });
  return <div />;
}

// Violation: the loop's iterable is evaluated once, before its first await.
export function LoopIterableRead() {
  const [count] = createSignal(0);
  now(async () => {
    for (const x of [count()]) {
      await p;
      void x;
    }
  });
  return <div />;
}

// Uncertifiable: `finally` runs after the try block's await.
export function TryFinallyRead() {
  const [count] = createSignal(0);
  now(async () => {
    try {
      await p;
    } finally {
      console.log(count());
    }
  });
  return <div />;
}

// Uncertifiable: `catch` runs after the awaited promise rejects.
export function TryCatchRead() {
  const [count] = createSignal(0);
  now(async () => {
    try {
      await p;
    } catch {
      console.log(count());
    }
  });
  return <div />;
}

// Silent: the await is unconditional, so the read is past the window.
export function UnconditionalAwaitRead() {
  const [count] = createSignal(0);
  now(async () => {
    await p;
    console.log(count());
  });
  return <div />;
}

// Violation: the read precedes the conditional await.
export function ReadBeforeIfAwait() {
  const [count] = createSignal(0);
  now(async () => {
    console.log(count());
    if (c) await p;
  });
  return <div />;
}

// Uncertifiable: a helper called after a conditional await reads on a call
// that may run after it.
export function HelperAfterIfAwait() {
  const [count] = createSignal(0);
  function read() {
    return count();
  }
  now(async () => {
    if (c) await p;
    read();
  });
  return <div />;
}

// Violation: the helper is called before the conditional await.
export function HelperBeforeIfAwait() {
  const [count] = createSignal(0);
  function read() {
    return count();
  }
  now(async () => {
    read();
    if (c) await p;
  });
  return <div />;
}

// Uncertifiable: a helper called in a loop before the loop's await runs after
// that await on a later iteration.
export function HelperInAwaitingLoop() {
  const [count] = createSignal(0);
  function read() {
    return count();
  }
  now(async () => {
    for (let i = 0; i < 2; i++) {
      if (i > 0) read();
      await p;
    }
  });
  return <div />;
}

// Uncertifiable: a `for await` later in the loop suspends before the next
// iteration's read.
export function ForAwaitBackEdge() {
  const [count] = createSignal(0);
  now(async () => {
    let i = 0;
    while (i < 2) {
      console.log(count());
      for await (const y of ys) {
        void y;
      }
      i++;
    }
  });
  return <div />;
}

// Uncertifiable: a destructuring default runs after the awaited value, though
// it is written first.
export function PatternDefaultAfterAwait() {
  const [count] = createSignal(0);
  now(async () => {
    const { a = count() } = await q;
    void a;
  });
  return <div />;
}

// Uncertifiable: the same for a destructuring assignment.
export function PatternAssignDefaultAfterAwait() {
  const [count] = createSignal(0);
  now(async () => {
    let a: number | undefined;
    ({ a = count() } = await q);
    void a;
  });
  return <div />;
}

// Uncertifiable: a store path read after a conditional await.
export function StoreAfterIfAwait() {
  const [store] = createStore({ value: 1 });
  now(async () => {
    if (c) await p;
    console.log(store.value);
  });
  return <div />;
}

// Violation: the await belongs to a nested closure, so it does not come
// before the read in this function.
export function NestedClosureAwait() {
  const [count] = createSignal(0);
  now(async () => {
    const later = async () => {
      await p;
    };
    void later;
    console.log(count());
  });
  return <div />;
}

// Uncertifiable: a nested closure is called after a conditional await, so its
// read runs on a call that may follow the suspension.
export function NestedAfterIfAwait() {
  const [count] = createSignal(0);
  now(async () => {
    const f = () => count();
    if (c) await p;
    f();
  });
  return <div />;
}

// Uncertifiable: the same closure, called in a loop before the loop's await,
// runs after that await on a later iteration.
export function NestedInAwaitingLoop() {
  const [count] = createSignal(0);
  now(async () => {
    for (let i = 0; i < 2; i++) {
      const f = () => count();
      if (i > 0) f();
      await p;
    }
  });
  return <div />;
}

// Violation: the closure is called before any await.
export function NestedBeforeAwait() {
  const [count] = createSignal(0);
  now(async () => {
    const f = () => count();
    f();
    await p;
  });
  return <div />;
}

declare const xs: Promise<{ a?: number }[]>;

// Uncertifiable: a `for…of` pattern default runs after the await that produces
// the iterable.
export function ForOfPatternAfterAwait() {
  const [count] = createSignal(0);
  now(async () => {
    for (const { a = count() } of await xs) {
      void a;
    }
  });
  return <div />;
}
