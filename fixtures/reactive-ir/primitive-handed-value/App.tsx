import { createEffect, createMemo, flush, resolve, untrack } from "solid-js";

function run(fn: () => void) {
  fn();
}

// Batch 11 (ADR 0825 amended; the held patterns `byvalue/*`). A Solid primitive handed to a call by value is run by the call's callee, and the frame
// Chrome (rc.21) reports for that run stands at the call (V8 puts it at the call's start): `untrack(flush)` calls flush on the effect's stack, so does
// `run(flush)`, a callback `forEach` runs, a spread argument, a value held in a map the call constructs. The `primitive-value` row (SC9012) stands at the
// argument, and the judges cover a withholding at a frame's position only: the programs were judged false clean. A companion row (`primitive-handed`)
// stands at the start of the call the primitive is handed to, for a primitive the binder confirmed: uncertifiable, never a violation.

// An effect's apply (FLUSH_IN_EFFECT_CALLBACK): each form hands `flush` to a call.
export function Untracked() {
  createEffect(() => 1, () => {
    untrack(flush);
  });
  return <div />;
}

export function Helper() {
  createEffect(() => 1, () => {
    run(flush);
  });
  return <div />;
}

export function ForEachCallback() {
  createEffect(() => 1, () => {
    [1].forEach(flush);
  });
  return <div />;
}

export function Spread() {
  createEffect(() => 1, () => {
    run(...[flush]);
  });
  return <div />;
}

export function Stored() {
  createEffect(() => 1, () => {
    new Map([["k", flush]]).get("k")!();
  });
  return <div />;
}

// A memo (RESOLVE_IN_REACTIVE_SCOPE): `resolve` handed to a project function.
function runResolve(fn: (f: () => number) => Promise<number>, f: () => number) {
  void fn(f);
}

export function Resolving() {
  const m = createMemo(() => {
    n();
    runResolve(resolve, () => n());
    return 1;
  });
  return <p>{m()}</p>;
}

// Twins Chrome keeps quiet on (the call defers the run): each hands the primitive to a call too, so each has its rows, and none a violation.
export function Deferred() {
  createEffect(() => 1, () => {
    queueMicrotask(flush);
    Promise.resolve().then(flush);
    setTimeout(flush);
  });
  return <div />;
}
