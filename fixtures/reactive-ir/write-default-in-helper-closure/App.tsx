import { createMemo, createSignal } from "solid-js";

// A parameter's default runs only when the caller leaves the argument out.
function runDefault(cb: () => void, x: unknown = cb()) {
  return x;
}
function runBody(cb: () => void) {
  cb();
  return 1;
}
// `g` is built and returned, never called here: the default never runs.
function runNested(cb: () => void) {
  const g = (x: unknown = cb()) => x;
  return g;
}

// Uncertifiable: whether the default runs depends on the argument.
export function DefaultOmitted() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    runDefault(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}

// Uncertifiable: the argument is supplied, so the callback never runs
// (rc.17: nothing throws).
export function DefaultSupplied() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    runDefault(() => setV(1), 5);
    return 1;
  });
  return <p>{m()}{v()}</p>;
}

// Violation: a call in the body of the helper runs the callback in the memo.
export function BodyCall() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    runBody(() => setV(1));
    return 1;
  });
  return <p>{m()}{v()}</p>;
}

// KNOWN FALSE POSITIVE, pinned as the current (wrong) answer: `runNested`
// returns a closure whose default calls `cb`, and nothing calls that closure,
// so the write never runs and rc.17 throws nothing. The lookups behind
// `calls_parameter_in_body` and the every-completion table place a call in a
// nested parameter list in the function around it (docs/open-work.md). The
// fix moves this to uncertifiable and shows in this snapshot.
export function NestedDefaultNeverRuns() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    runNested(() => setV(1));
    return 1;
  });
  return <button onClick={() => m()}>{v()}</button>;
}
