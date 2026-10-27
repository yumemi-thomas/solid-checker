import { createMemo, createRoot, createSignal, untrack } from "solid-js";

// A closure a one-shot scope builds (`createRoot`, `untrack`) runs where it is
// called, and a default value in its parameter list runs only when a call
// omits the argument: building the closure runs nothing (ADR 0455, 0454). The
// call below happens in a click handler, where the write is legal.

export function RootBuildsClosure() {
  const [v, setV] = createSignal(0);
  const api = createRoot(() => {
    return { f: (p: number = setV(1)) => p };
  });
  return <button onClick={() => api.f()}>{v()}</button>;
}
export function RootDestructuringDefault() {
  const [v, setV] = createSignal(0);
  const run = createRoot(() => {
    const inner = ({ p = setV(1) }: { p?: number } = {}) => p;
    const middle = () => inner();
    function outer() {
      return middle();
    }
    return outer;
  });
  return <button onClick={() => run()}>{v()}</button>;
}
export function RootIifeInDefault() {
  const [v, setV] = createSignal(0);
  const run = createRoot(() => {
    const f = (p: unknown = (({ q = setV(1) }: { q?: number } = {}) => q)()) => p;
    return f;
  });
  return <button onClick={() => run()}>{v()}</button>;
}
export function UntrackBuildsClosure() {
  const [v, setV] = createSignal(0);
  const api = untrack(() => {
    return { f: (p: number = setV(1)) => p };
  });
  return <button onClick={() => api.f()}>{v()}</button>;
}
export function UntrackChain() {
  const [v, setV] = createSignal(0);
  const run = untrack(() => {
    const f1 = (p: number = setV(1)) => p;
    const f2 = (p: unknown = f1()) => p;
    return f2;
  });
  return <button onClick={() => run()}>{v()}</button>;
}

// Costs: the closure is called in the root or untrack body with the argument
// omitted. The write runs there and rc.17 throws, but nothing proves the
// omission (a closure with code in its parameter list has no call sites, ADR
// 0339): uncertifiable, as in a memo.
export function RootCallsDefault() {
  const [v, setV] = createSignal(0);
  createRoot(() => {
    const f = (p: number = setV(1)) => p;
    f();
  });
  return <div>{v()}</div>;
}
export function RootIifeDefault() {
  const [v, setV] = createSignal(0);
  createRoot(() => {
    ((p: number = setV(1)) => p)();
  });
  return <div>{v()}</div>;
}
export function UntrackIifeDefault() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    ((p: number = setV(1)) => p)();
  });
  return <div>{v()}</div>;
}

// Controls: the same shape in a memo (ADR 0452, uncertifiable), and a write in
// the body of a root's own code, which rc.17 rejects (violations).
export function MemoBuildsClosure() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    return { f: (p: number = setV(1)) => p };
  });
  return <button onClick={() => m().f()}>{v()}</button>;
}
export function RootBodyClosureCall() {
  const [v, setV] = createSignal(0);
  createRoot(() => {
    const f = () => setV(1);
    f();
  });
  return <div>{v()}</div>;
}
export function RootBodyForEach() {
  const [v, setV] = createSignal(0);
  createRoot(() => {
    [1].forEach(() => {
      setV(1);
    });
  });
  return <div>{v()}</div>;
}
export function UntrackBodyWrite() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    setV(1);
  });
  return <div>{v()}</div>;
}
// The root's closure body writes, and is called in a click handler: legal, and
// no call in the root proves otherwise.
export function RootBuildsBodyWrite() {
  const [v, setV] = createSignal(0);
  const api = createRoot(() => {
    return { f: () => setV(1) };
  });
  return <button onClick={() => api.f()}>{v()}</button>;
}
