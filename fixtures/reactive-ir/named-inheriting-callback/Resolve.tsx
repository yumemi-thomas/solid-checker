import { createMemo, createSignal, resolve } from "solid-js";

// Violation (SC2004, rc.17 in Chrome: RESOLVE_IN_REACTIVE_SCOPE): resolve() in a
// closure a memo compute named by its binding runs in place.
export function MemoResolveClosure() {
  const compute = () => {
    const wait = () => { resolve(() => 1); };
    wait();
    return 1;
  };
  const m = createMemo(compute);
  return <div><p>{m()}</p></div>;
}

// Clean: a tracked read in the same layer.
export function MemoReadClosure() {
  const [s] = createSignal(0);
  const compute = () => { const read = () => s(); return read(); };
  const m = createMemo(compute);
  return <div><p>{m()}</p></div>;
}

// Uncertifiable: resolve() in `[1].forEach` inside the named compute (a library
// callback may run zero times; rc.17 throws RESOLVE_IN_REACTIVE_SCOPE here).
export function MemoResolveForEach() {
  const compute = () => {
    [1].forEach(() => { resolve(() => 1); });
    return 1;
  };
  const m = createMemo(compute);
  return <div><p>{m()}</p></div>;
}
