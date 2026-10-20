import { createEffect, createRenderEffect, createSignal, flush, untrack } from "solid-js";
import { commitFromOtherFile } from "./helper";

// rc.13 dev raises FLUSH_IN_EFFECT_CALLBACK (a warning) when `flush` runs on
// the stack of a createEffect apply: the flush that runs effects is already in
// progress. Positives are the calls SC2008 claims; every other export is a
// control that must stay quiet at runtime and in the checker.

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

export function OtherFile() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    commitFromOtherFile();
  });
  return <span>other-file</span>;
}

export function Inline() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    (() => {
      flush();
    })();
    untrack(() => flush());
    [1].forEach(() => {
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

// Controls: no FLUSH_IN_EFFECT_CALLBACK.

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
