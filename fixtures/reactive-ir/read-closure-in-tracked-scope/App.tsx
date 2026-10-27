import { createMemo, createSignal } from "solid-js";

// A closure a tracked scope builds is not run by it. A read in the closure is
// tracked only where something calls the closure inside a tracking pass; the
// scope's own role says nothing about it. rc.17 warns STRICT_READ_UNTRACKED at
// the `count()` of Named, Anonymous and Stored (probed), each called in the
// component body through the memo's value.

export function Named() {
  const [count] = createSignal(1);
  const m = createMemo(() => {
    const f = () => count();
    return f;
  });
  const v = m()();
  return <div>{v}</div>;
}

export function Anonymous() {
  const [count] = createSignal(1);
  const m = createMemo(() => () => count());
  const v = m()();
  return <div>{v}</div>;
}

export function Stored() {
  const [count] = createSignal(1);
  const m = createMemo(() => ({ f: () => count() }));
  const v = m().f();
  return <div>{v}</div>;
}

// Never called at all: still no placement.
export function NamedNeverCalled() {
  const [count] = createSignal(1);
  createMemo(() => {
    const f = () => count();
    void f;
    return 1;
  });
  return <div />;
}

// ---- Tracked: the closure is called in the scope's own pass, every reference
// being such a call.

export function NamedCalledInCompute() {
  const [count] = createSignal(1);
  const m = createMemo(() => {
    const f = () => count();
    return f() + 1;
  });
  return <div>{m()}</div>;
}

export function NamedDeclarationCalledInCompute() {
  const [count] = createSignal(1);
  const m = createMemo(() => {
    function f() {
      return count();
    }
    return f() + 1;
  });
  return <div>{m()}</div>;
}

export function IifeInCompute() {
  const [count] = createSignal(1);
  const m = createMemo(() => (() => count())() + 1);
  return <div>{m()}</div>;
}

// A recursive closure asks its own calls for their placement, so it is
// unplaced rather than looped over (a cost, not a claim).
export function RecursiveNamedClosure() {
  const [count] = createSignal(3);
  const m = createMemo(() => {
    const down = (n: number): number => (n > 0 ? down(n - 1) : count());
    return down(2);
  });
  return <div>{m()}</div>;
}

// ---- A default value is read when the function is called without the
// argument, not where the function is built. rc.17 warns at `count()` of each
// of these three (probed: two STRICT_READ_UNTRACKED, the control one).

export function ReturnedDefaultRead() {
  const [count] = createSignal(1);
  const m = createMemo(() => {
    const f = (x = count()) => x;
    return f;
  });
  const v = m()();
  return <div>{v}</div>;
}

export function ReturnedAnonymousDefaultRead() {
  const [count] = createSignal(1);
  const m = createMemo(() => (x = count()) => x);
  const v = m()();
  return <div>{v}</div>;
}

export function ReturnedDestructuringDefaultRead() {
  const [count] = createSignal(1);
  const m = createMemo(() => {
    const f = ({ a = count() }: { a?: number }) => a;
    return f;
  });
  const v = m()({});
  return <div>{v}</div>;
}

// Tracked: a default of a callback the scope runs in place.
export function InlineCallbackDefaultRead() {
  const [count] = createSignal(1);
  const m = createMemo(() => [undefined].map((x = count()) => x)[0]);
  return <div>{m()}</div>;
}

// A function written inside a default value is the closure's too.
export function ReturnedIifeDefaultRead() {
  const [count] = createSignal(1);
  const m = createMemo(() => {
    const f = (x = (() => count())()) => x;
    return f;
  });
  const v = m()();
  return <div>{v}</div>;
}
