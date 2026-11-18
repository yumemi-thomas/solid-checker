import { Loading, createEffect, createMemo, createRenderEffect } from "solid-js";

// ADR 1102: a call of a local function that reads a pending async memo (`wrapUse`, ADR 0510), made by a memo's compute or by exact in-place layers it
// runs, is the memo's own suspending read: a non-render computation raises no boundary code, and whoever reads the memo is a site of its own.

// Clean: called in the compute.
export function Direct() {
  const source = createMemo(async () => 1);
  const wrapUse = () => {
    source();
  };
  const view = createMemo(() => {
    wrapUse();
    return 1;
  });
  return (
    <Loading fallback={0}>
      <p>{view()}</p>
    </Loading>
  );
}

// Clean: called by a closure the compute calls once.
export function Closure() {
  const source = createMemo(async () => 1);
  const wrapUse = () => {
    source();
  };
  const view = createMemo(() => {
    const run = () => {
      wrapUse();
    };
    run();
    return 1;
  });
  return (
    <Loading fallback={0}>
      <p>{view()}</p>
    </Loading>
  );
}

// Clean: called by an IIFE in the compute.
export function Iife() {
  const source = createMemo(async () => 1);
  const wrapUse = () => {
    source();
  };
  const view = createMemo(() => {
    (() => {
      wrapUse();
    })();
    return 1;
  });
  return (
    <Loading fallback={0}>
      <p>{view()}</p>
    </Loading>
  );
}

// Not ended (twin): a render effect's compute is a render, where the boundary claim stands.
export function RenderCompute() {
  const source = createMemo(async () => 1);
  const wrapUse = () => {
    source();
  };
  createRenderEffect(
    () => {
      wrapUse();
      return 1;
    },
    () => {},
  );
  return <p>static</p>;
}

// Not ended (twin): an effect's apply is untracked.
export function ApplyCall() {
  const source = createMemo(async () => 1);
  const wrapUse = () => {
    source();
  };
  createEffect(
    () => 1,
    () => {
      wrapUse();
    },
  );
  return <p>static</p>;
}
