import { createMemo, createSignal } from "solid-js";
import { observe } from "dep-ext";

// Each helper may hand its parameter to `observe` (no analyzed body), but no
// link is proven: the callee value, the value reaching the argument, the
// call running, or the final callee being undescribed. SC9012 obligations
// at each `count`, never a violation (ADR 0370).
type S = () => number;
function wrap<T>(_f: T): T {
  return (() => {}) as unknown as T;
}
// Callee identity: a wrapper's result, a reassigned binding.
const wrapped = wrap((s: S) => observe(s));
let reassigned = (s: S) => {
  observe(s);
};
reassigned = () => {};
// Killing writes.
function overwritten(s: S) {
  s = () => 0;
  observe(s);
}
function overwrittenAlias(s: S) {
  let held = s;
  held = () => 0;
  observe(held);
}
// Calls that may not run.
function uncalledClosure(s: S) {
  const neverCalled = () => observe(s);
}
function deadBranch(s: S) {
  if (false) observe(s);
}
function afterReturn(s: S) {
  return;
  observe(s);
}
function guarded(s: S, go: boolean) {
  if (go) observe(s);
}
function unevaluatedDefault(s: S, unused: unknown = observe(s)) {}
// Callees that are not undescribed.
const api = {
  read(x: S) {
    return createMemo(x);
  },
};
function objectMethod(s: S) {
  return api.read(s);
}
function closureCallback(s: S, cb: (s: S) => void) {
  const inner = () => cb(s);
  inner();
}
const [n, setN] = createSignal(0);
function setter(s: S) {
  setN(s);
}

export function Panel() {
  const [count] = createSignal(1);
  wrapped(count);
  reassigned(count);
  overwritten(count);
  overwrittenAlias(count);
  uncalledClosure(count);
  deadBranch(count);
  afterReturn(count);
  guarded(count, false);
  unevaluatedDefault(count, null);
  objectMethod(count);
  closureCallback(count, (s) => {
    createMemo(s);
  });
  setter(count);
  return <p>{count() + n()}</p>;
}
