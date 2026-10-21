import { createEffect, createRenderEffect, createRoot, createSignal, flush, untrack } from "solid-js";
import { commitFromOtherFile } from "./helper";

// rc.13 dev raises FLUSH_IN_EFFECT_CALLBACK (a warning) when `flush` runs on
// the stack of a createEffect apply: the flush that runs effects is already in
// progress. Positives are the calls SC2008 claims; every other export is a
// control that must stay quiet at runtime and in the checker, except the
// documented static-only ones: the claim is that the call only ever runs on an
// apply's stack, so dead code is reported though the runtime never reaches it.

export function Direct() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    flush();
  });
  return <span>direct</span>;
}

export function WithCallback() {
  const [value, setValue] = createSignal(1);
  createEffect(value, () => {
    flush(() => setValue(2));
  });
  return <span>with-callback</span>;
}

function commitDeclared() {
  flush();
}
const outerArrow = () => commitDeclared();
export function Helpers() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    outerArrow();
  });
  return <span>helpers</span>;
}

// An exported helper may be called from anywhere, so the checker is silent
// though this call warns: a known open site (oracle.json).
export function OtherFile() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    commitFromOtherFile();
  });
  return <span>other-file</span>;
}

export function Inline() {
  const [value] = createSignal(1);
  const items = [1];
  createEffect(value, () => {
    (() => {
      flush();
    })();
    untrack(() => flush());
    items.forEach(() => {
      flush();
    });
  });
  return <span>inline</span>;
}

async function loadThenSettle() {
  flush();
  await Promise.resolve();
}
export function AsyncHelper() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    void loadThenSettle();
  });
  return <span>async-helper</span>;
}

// Documented static-only: the checker claims the no-op (the queue is already
// running), and the dev warning is absent because a synchronous nested
// `runEffect` lowers the global `inEffectCallback` marker in its `finally`
// (rc.13 `dist/dev.js:1906`) before the flush; the conditional never runs.

export function RenderThenFlush() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    createRenderEffect(
      () => 1,
      () => {},
    );
    flush();
  });
  return <span>render-then-flush</span>;
}

export function JsxThenFlush() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    const el = createRoot(() => <div>{value()}</div>);
    void el;
    flush();
  });
  return <span>jsx-then-flush</span>;
}

function mount() {
  createRoot(() => {
    createRenderEffect(
      () => 2,
      () => {},
    );
  });
}
export function HelperThenFlush() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    mount();
    flush();
  });
  return <span>helper-then-flush</span>;
}

export function FlushFnRender() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    flush(() => {
      createRenderEffect(
        () => 3,
        () => {},
      );
    });
  });
  return <span>flush-fn-render</span>;
}

export function Conditional() {
  const [value] = createSignal(1);
  createEffect(value, (next) => {
    if (next > 100) flush();
  });
  return <span>conditional</span>;
}

// Controls: no FLUSH_IN_EFFECT_CALLBACK at runtime. AfterReturn, EmptyLiteral,
// ThrowingCompute, TryReturn and NeverBeforeFlush are dead code the checker
// reports anyway (static-only).

export function AfterReturn() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    return;
    flush();
  });
  return <span>after-return</span>;
}

export function EmptyLiteral() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    ([] as number[]).forEach(() => flush());
  });
  return <span>empty-literal</span>;
}

export function ThrowingCompute() {
  createEffect(
    (): number => {
      throw new Error("x");
    },
    () => {
      flush();
    },
  );
  return <span>throwing-compute</span>;
}

export function InHandler() {
  return <button onClick={() => flush()}>handler</button>;
}

export function Deferred() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    setTimeout(() => flush());
    queueMicrotask(() => flush());
  });
  return <span>deferred</span>;
}

export function Shadowed() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    const flush = () => {};
    flush();
  });
  return <span>shadowed</span>;
}

export function ComputeFunction() {
  const [value] = createSignal(1);
  createEffect(
    () => {
      flush();
      return value();
    },
    () => {},
  );
  return <span>compute</span>;
}

// A render effect's first run is synchronous at creation, with no flush in
// progress: `flush` drains there, and the effect never re-runs.
export function RenderEffect() {
  createRenderEffect(
    () => 1,
    () => {
      flush();
    },
  );
  return <span>render-effect</span>;
}

export function NeverApplied() {
  createEffect(
    () => 1,
    () => {
      flush();
    },
    { defer: true },
  );
  return <span>never-applied</span>;
}

export function TryReturn() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    try {
      return;
    } finally {
      void 0;
    }
    flush();
  });
  return <span>try-return</span>;
}

function fail(): never {
  throw new Error("always");
}
export function NeverBeforeFlush() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    try {
      fail();
    } catch {}
    return;
    flush();
  });
  return <span>never-before-flush</span>;
}
