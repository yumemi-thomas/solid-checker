import { createMemo, createSignal } from "solid-js";

declare global {
  interface Window {
    ranNested: number;
    ranForward: number;
    ranConditional: number;
    ranDirect: number;
    ranCalled: number;
    ranSupplied: number;
    ranForEach: number;
  }
}
window.ranNested = 0;
window.ranForward = 0;
window.ranConditional = 0;
window.ranDirect = 0;
window.ranCalled = 0;
window.ranSupplied = 0;
window.ranForEach = 0;

function runBody(cb: () => void) {
  cb();
  return 1;
}
function buildNested(cb: () => void) {
  const g = (x: unknown = cb()) => x;
  return g;
}
function buildForward(cb: () => void) {
  const g = (x: unknown = runBody(cb)) => x;
  return g;
}
function buildConditional(cb: () => void, flag: boolean) {
  if (flag) {
    const g = (x: unknown = cb()) => x;
    return g;
  }
  return undefined;
}
function buildDirect(cb: () => void) {
  return (x: unknown = cb()) => x;
}
function buildAndCall(cb: () => void) {
  const g = (x: unknown = cb()) => x;
  return g();
}
function buildAndCallSupplied(cb: () => void) {
  const g = (x: unknown = cb()) => x;
  return g(5);
}

// A closure with a call in its parameter list is built and never called: the
// default does not run, nothing throws.
export function Nested() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildNested(() => { window.ranNested++; setV(1); });
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function Forward() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildForward(() => { window.ranForward++; setV(1); });
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function Conditional() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildConditional(() => { window.ranConditional++; setV(1); }, true);
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function Direct() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildDirect(() => { window.ranDirect++; setV(1); });
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
// Called without the argument: the default runs in the memo and the write
// throws. Called with it: nothing runs.
export function CalledOmitted() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildAndCall(() => { window.ranCalled++; setV(1); });
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
export function CalledSupplied() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    buildAndCallSupplied(() => { window.ranSupplied++; setV(1); });
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
// `forEach` supplies the index, so the default of an anonymous callback never runs.
export function ForEachSupplied() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    [1].forEach((item, index = (window.ranForEach++, setV(1))) => void item);
    return 1;
  });
  return <p>{m()}{v()}</p>;
}
