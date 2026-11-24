import { action, createEffect, createMemo, createSignal, createTrackedEffect, flush } from "solid-js";

const [n] = createSignal(0);

// ADR 1201: the program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason holds and each flush stands on where it is
// placed. No `await` dominates these flushes, yet each runs only past a suspension of its own async function: the census places them as it places code after
// an await.
export const step = action(function* () {
  yield 1;
});

export function Host() {
  createEffect(() => n(), () => {});
  createTrackedEffect(() => {
    n();
  });
  return <div />;
}

// Clean: a counted loop's later pass (the `i > 0` consequent; the body awaits on every pass) in an async memo.
export function MemoLoop() {
  const m1 = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        flush();
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m1(), () => {});
  return <div />;
}

// Clean: the `finally` behind a `try` head's `await Promise.resolve()` in an async effect compute.
export function EffectFinally() {
  createEffect(
    async () => {
      try {
        await Promise.resolve();
      } finally {
        flush();
      }
      return 1;
    },
    () => {},
  );
  return <div />;
}

// Clean: a helper called only in a counted loop's later pass (its one call is asked as the site: ADR 1199).
function later() {
  flush();
}
export function HelperLoop() {
  const m2 = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        later();
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m2(), () => {});
  return <div />;
}

// Clean: an event handler's async function, its flush in a closure called in the later pass (the census class of the call is the delivery's,
// and the call still runs past the function's own await).
export function HandlerLoop() {
  const run = async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        const f1 = () => {
          flush();
        };
        f1();
      }
      await Promise.resolve();
    }
  };
  return <button onClick={() => run()}>go</button>;
}

// Clean: the memo's compute handed by name, its flush in the later pass (slice 2: the effect-phase walk's unplaced row asks the same proof).
export function NamedLoop() {
  const cb1 = async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        flush();
      }
      await Promise.resolve();
    }
    return 1;
  };
  const m5 = createMemo(cb1);
  createEffect(() => m5(), () => {});
  return <div />;
}

// Clean: an effect's compute handed by name, its flush in the `finally` behind the `try` head's await.
export function NamedFinally() {
  const cb2 = async () => {
    try {
      await Promise.resolve();
    } finally {
      flush();
    }
    return 1;
  };
  createEffect(cb2, () => {});
  return <div />;
}

// Uncertifiable (fail closed): a compute handed by name that flushes on its first pass.
export function NamedFirstPass() {
  const cb3 = async () => {
    for (let i = 0; i < 2; i += 1) {
      flush();
      await Promise.resolve();
    }
    return 1;
  };
  const m6 = createMemo(cb3);
  createEffect(() => m6(), () => {});
  return <div />;
}

// Uncertifiable (fail closed): the same handler's function flushing on its first pass, on the delivery's stack.
export function HandlerFirstPass() {
  const run = async () => {
    for (let i = 0; i < 2; i += 1) {
      flush();
      await Promise.resolve();
    }
  };
  return <button onClick={() => run()}>go</button>;
}

// Uncertifiable (fail closed): no `i > 0` guard, so the first pass flushes before any await, on the compute's stack.
export function FirstPass() {
  const m3 = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      flush();
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m3(), () => {});
  return <div />;
}

// Uncertifiable (fail closed): a `break` in the body, so a later pass need not follow a completed await.
export function Breaks() {
  const m4 = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        flush();
      }
      if (n() > 5) break;
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m4(), () => {});
  return <div />;
}

// Uncertifiable (fail closed): the `try` head awaits a call that may throw before it suspends, so the `finally` may run on the compute's stack.
function risky(): Promise<void> {
  if (n() > 5) throw new Error("now");
  return Promise.resolve();
}
export function RiskyFinally() {
  createEffect(
    async () => {
      try {
        await risky();
      } finally {
        flush();
      }
      return 1;
    },
    () => {},
  );
  return <div />;
}
