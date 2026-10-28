import { createMemo, createRoot, createTrackedEffect, isPending, onSettled } from "solid-js";

// ADR 0490: an async memo read in a named local function is read where a
// call of that function runs. A call in the component body's strict-read
// window or in a leaf owner reads there unsuspendably, as a direct read does.

// Violation: a closure called in the component body.
export function BodyClosure() {
  const user = createMemo(async () => 1);
  const f1 = () => {
    user();
  };
  f1();
  return <p />;
}

// Violation: two named layers, the outer called in the body.
export function BodyTwoLayers() {
  const user = createMemo(async () => 1);
  const f1 = () => {
    user();
  };
  const f2 = () => {
    f1();
  };
  f2();
  return <p />;
}

// Violation: a wrapper called in a tracked effect.
export function TrackedWrapper() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  createTrackedEffect(() => {
    wrapUse();
  });
  return <p />;
}

// Violation: a wrapper called in an onSettled callback.
export function SettledWrapper() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  onSettled(() => {
    wrapUse();
  });
  return <p />;
}

// Negative: the closure is called only from an event handler.
export function InHandler() {
  const user = createMemo(async () => 1);
  const f1 = () => {
    user();
  };
  return <button onClick={() => f1()} />;
}

// Negative: the closure is called under `isPending`.
export function Probed() {
  const user = createMemo(async () => 1);
  const f1 = () => user();
  return <p>{String(isPending(() => f1()))}</p>;
}

// Negative: the closure is never called.
export function NeverCalled() {
  const user = createMemo(async () => 1);
  const f1 = () => {
    user();
  };
  void [f1];
  return <p />;
}

// Negative: a written binding is not followed.
export function Reassigned() {
  const user = createMemo(async () => 1);
  let f1 = () => {
    user();
  };
  f1 = () => {};
  f1();
  return <p />;
}

// Violation (ADR 0496): a closure called in a createRoot callback the body
// runs, which reads untracked as the body does.
export function RootClosure() {
  const user = createMemo(async () => 1);
  createRoot(() => {
    const f1 = () => {
      user();
    };
    f1();
  });
  return <p />;
}

// Negative: the closure is called in a memo compute, which suspends.
export function MemoClosure() {
  const user = createMemo(async () => 1);
  const total = createMemo(() => {
    const f1 = () => user();
    return f1();
  });
  return <p>{total()}</p>;
}

// Negative: under the root, the closure is called only from a timer.
export function RootTimer() {
  const user = createMemo(async () => 1);
  createRoot(() => {
    const f1 = () => {
      user();
    };
    setTimeout(f1, 0);
  });
  return <p />;
}
