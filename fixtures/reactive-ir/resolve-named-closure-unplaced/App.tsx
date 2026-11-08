import { createMemo, createSignal, resolve, until, untrack } from "solid-js";

declare function run(f: () => void): void;

function wrapUse() { resolve(() => 1); }

// Violation (ADR 0857; rc.17 throws RESOLVE_IN_REACTIVE_SCOPE): a closure handed by
// name to `[1].forEach` inside a compute a memo names by its binding.
export function NamedForEach() {
  const compute = () => {
    const hf = () => { resolve(() => 1); };
    [1].forEach(hf);
    return 1;
  };
  const m = createMemo(compute);
  return <div><p>{m()}</p></div>;
}

// Violation (ADR 0857): the same through a `const` alias of the compute (until).
export function AliasForEachUntil() {
  const inner = () => {
    const hf = () => { void until(() => true); };
    [1].forEach(hf);
    return 1;
  };
  const compute = inner;
  const m = createMemo(compute);
  return <div><p>{m()}</p></div>;
}

// Uncertifiable: the compute named at two memos, the closure handed to a
// function this file does not see.
export function SharedUnknownRun() {
  const compute = () => {
    const hf = () => { resolve(() => 1); };
    run(hf);
    return 1;
  };
  const m = createMemo(compute);
  createMemo(compute);
  return <div><p>{m()}</p></div>;
}

// Uncertifiable: a module helper called from a closure an in-place local
// helper runs (`run2(hf)`), in a shared named compute.
export function SharedWrapperLocalHelper() {
  const compute = () => {
    const hf = () => { wrapUse(); };
    const run2 = (g: () => void) => { g(); };
    run2(hf);
    return 1;
  };
  const m = createMemo(compute);
  createMemo(compute);
  return <div><p>{m()}</p></div>;
}

// Violation (unchanged): resolve() directly in an aliased named compute.
export function AliasDirect() {
  const inner = () => { resolve(() => 1); return 1; };
  const compute = inner;
  const m = createMemo(compute);
  return <div><p>{m()}</p></div>;
}

// Clean: untrack clears the observer, also in a named compute.
export function NamedUntrack() {
  const compute = () => untrack(() => { resolve(() => 1); return 1; });
  const m = createMemo(compute);
  return <div><p>{m()}</p></div>;
}

// Uncertifiable (SC1001), as its inline-compute twin: a read in a closure
// handed to a local helper the role does not follow.
export function NamedReadLocalHelper() {
  const [s] = createSignal(0);
  const compute = () => {
    let out = 0;
    const read = () => { out = s(); };
    const run2 = (g: () => void) => { g(); };
    run2(read);
    return out;
  };
  const m = createMemo(compute);
  return <div><p>{m()}</p></div>;
}

// Clean: resolve() in a handler of the component body.
export function HandlerResolve() {
  const [s] = createSignal(0);
  return <button onClick={() => { void resolve(() => s()); }}>go</button>;
}
