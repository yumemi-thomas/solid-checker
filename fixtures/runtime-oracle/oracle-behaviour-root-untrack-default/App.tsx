import { createRoot, createSignal, untrack } from "solid-js";

declare global {
  interface Window {
    landedRootApi: number;
    landedRootChain: number;
    landedUntrackApi: number;
    landedUntrackChain: number;
  }
}
window.landedRootApi = 0;
window.landedRootChain = 0;
window.landedUntrackApi = 0;
window.landedUntrackChain = 0;

// A closure a one-shot scope builds runs nothing there; its default runs when a
// click calls it without the argument, in a handler where the write is legal
// (ADR 0455). The counter in the default is bumped after the write, so it is
// 1 only when the write did not throw.
export function RootBuildsClosure() {
  const [v, setV] = createSignal(0);
  const api = createRoot(() => {
    return { f: (p: number = (setV(1), window.landedRootApi++)) => p };
  });
  return <button data-act="root" onClick={() => api.f()}>{v()}</button>;
}
export function RootChain() {
  const [v, setV] = createSignal(0);
  const run = createRoot(() => {
    const inner = ({ p = (setV(1), window.landedRootChain++) }: { p?: number } = {}) => p;
    const middle = () => inner();
    function outer() {
      return middle();
    }
    return outer;
  });
  return <button data-act="rootchain" onClick={() => run()}>{v()}</button>;
}
export function UntrackBuildsClosure() {
  const [v, setV] = createSignal(0);
  const api = untrack(() => {
    return { f: (p: number = (setV(1), window.landedUntrackApi++)) => p };
  });
  return <button data-act="untrack" onClick={() => api.f()}>{v()}</button>;
}
export function UntrackChain() {
  const [v, setV] = createSignal(0);
  const run = untrack(() => {
    const f1 = (p: number = (setV(1), window.landedUntrackChain++)) => p;
    const f2 = (p: unknown = f1()) => p;
    return f2;
  });
  return <button data-act="untrackchain" onClick={() => run()}>{v()}</button>;
}

// Costs: the default runs in the root or untrack body, where rc.17 throws
// (REACTIVE_WRITE_IN_OWNED_SCOPE). The finding is uncertifiable, not a
// violation, and covers the dev diagnostic.
export function RootCallsDefault() {
  const [v, setV] = createSignal(0);
  createRoot(() => {
    const f = (p: number = setV(1)) => p;
    f();
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
