import { createRenderEffect, createSignal } from "solid-js";

declare const flag: boolean;
const later: { defer: boolean } = { defer: false };

// ADR 0503: a render effect's first apply runs during the call, under the
// component's owner, when the component is rendered by a module-scope
// `render` (main.tsx), the call has no options, and its compute settles on
// its first pass; a write there throws REACTIVE_WRITE_IN_OWNED_SCOPE.

// Violation: mounted by `render`, the apply writes a signal.
export function FirstRunWrite() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}

// Violation: the write is in a closure the apply calls in place.
export function FirstRunWriteLayered() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      const write = () => {
        setCount(1);
      };
      write();
    },
  );
  return <p>{count()}</p>;
}

// Uncertifiable: no mount is proven (not rendered by main.tsx).
export function Unmounted() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}

// Uncertifiable: `schedule` queues the first run (ADR 0502).
export function Scheduled() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(1);
    },
    { schedule: true },
  );
  return <p>{count()}</p>;
}

// Uncertifiable: options the analysis cannot read may defer it.
export function OptionsVariable() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(1);
    },
    later,
  );
  return <p>{count()}</p>;
}

// Uncertifiable: the compute reads a source, which may be pending.
export function ComputeReads() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => count(),
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}

// Uncertifiable: the effect is created on one path of the body.
export function ConditionalEffect() {
  const [count, setCount] = createSignal(0);
  if (flag) {
    createRenderEffect(
      () => 1,
      () => {
        setCount(1);
      },
    );
  }
  return <p>{count()}</p>;
}

// Uncertifiable: `hydrate` may run the component later (main.tsx).
export function Hydrated() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}

// Uncertifiable: rendered from inside a function, not at module scope.
export function RenderedLater() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}

// ADR 0503 review negatives (uncertifiable): computes that are no single
// primitive literal.
export function ComputeImports() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => import("./App"),
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}

export function ComputeThenable() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => ({ then() {} }),
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}

export function ComputeDestructures() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    ({ a }: any) => 1,
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}

// A binding reassigned before the render mounts another function.
export let Replaced = () => {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
};
Replaced = () => <p />;

// Rendered from a class field and under a condition: not on every module run.
export function InClassField() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}

export function UnderCondition() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      setCount(1);
    },
  );
  return <p>{count()}</p>;
}
