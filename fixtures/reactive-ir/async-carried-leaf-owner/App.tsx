import { createMemo, createTrackedEffect, onSettled } from "solid-js";

// Violation: a named callback handed to onSettled runs `hf` through a
// carrier that provably runs it, so the pending read is under onSettled.
export function SettledNamedForEach() {
  const a1 = createMemo(async () => 1);
  const cb = () => {
    const hf = () => {
      a1();
    };
    [1].forEach(hf);
  };
  onSettled(cb);
  return <div />;
}

// Violation: the same callback handed to two calls (one const, shared).
export function SettledSharedCall() {
  const a2 = createMemo(async () => 1);
  const cb = () => {
    const hf = () => {
      a2();
    };
    hf.call(null);
  };
  onSettled(cb);
  onSettled(cb);
  return <div />;
}

// Violation: a tagged template runs its tag; the leaf owner is the tracked effect's.
export function TrackedNamedTagged() {
  const a3 = createMemo(async () => 1);
  const cb = () => {
    const hf = (_parts: TemplateStringsArray) => {
      a3();
    };
    hf`x`;
  };
  createTrackedEffect(cb);
  return <div />;
}

// Two carriers deep: `outer` runs `hf`, a running carrier runs `outer`.
export function SettledTwoCarriers() {
  const a4 = createMemo(async () => 1);
  const cb = () => {
    const hf = () => {
      a4();
    };
    const outer = () => {
      Array.from([1], hf);
    };
    Reflect.apply(outer, undefined, []);
  };
  onSettled(cb);
  return <div />;
}

// Uncertifiable: an empty receiver may run `hf` zero times.
export function SettledNamedEmpty() {
  const a5 = createMemo(async () => 1);
  const cb = () => {
    const hf = () => {
      a5();
    };
    ([] as number[]).forEach(hf);
  };
  onSettled(cb);
  return <div />;
}

// Uncertifiable (was a proven violation): the onSettled literal is around
// `hf` lexically, but `hf` runs only where the empty receiver runs it.
export function SettledInlineEmpty() {
  const a6 = createMemo(async () => 1);
  onSettled(() => {
    const hf = () => {
      a6();
    };
    ([] as number[]).forEach(hf);
  });
  return <div />;
}

// Not a violation: a project `forEach` is no carrier.
export function SettledProjectForEach() {
  const a7 = createMemo(async () => 1);
  const box = { forEach(run: () => void) { run(); } };
  const cb = () => {
    const hf = () => {
      a7();
    };
    box.forEach(hf);
  };
  onSettled(cb);
  return <div />;
}

// Silent: the named function runs in a memo's compute, which suspends.
export function MemoNamedForEach() {
  const a8 = createMemo(async () => 1);
  const cb = () => {
    const hf = () => {
      a8();
    };
    [1].forEach(hf);
    return 1;
  };
  const m = createMemo(cb);
  return <div>{m()}</div>;
}
