import { createSignal } from "solid-js";
import { lazyTracked, lazyUntracked, lazyMixed, lazyOpen, returnedCalc } from "reactive-package";
import * as RP from "reactive-package";

export function Correct() {
  const [source] = createSignal(1);
  const value = lazyTracked((() => source() * 2) satisfies () => number);
  return <div>{value()}</div>;
}

export function Misuse() {
  const [source] = createSignal(1);
  const value = lazyTracked(() => source() * 2);
  const stale = value();
  return <div>{stale}</div>;
}

export function MustRemainUncertifiable() {
  const [source] = createSignal(1);
  const a = lazyUntracked(() => source());
  const b = lazyMixed(() => source());
  const c = lazyOpen(() => source());
  return <div>{a() + b() + c()}</div>;
}

export function NestedEscape() {
  const [source] = createSignal(1);
  const value = lazyTracked(() => () => source());
  return <div>{value()()}</div>;
}

// Execution is still optional: neither the read proof nor callback closure
// may upgrade this unforced write to a proven owned-write violation.
export function OptionalWrite() {
  const [source, setSource] = createSignal(1);
  lazyTracked(() => { setSource(2); return source(); });
  return <div />;
}

export function Shadowed() {
  const [source] = createSignal(1);
  const lazyTracked = (fn: () => number) => fn;
  const value = lazyTracked(() => source());
  return <div>{value()}</div>;
}

export function NamespaceCorrect() {
  const [source] = createSignal(1);
  const value = RP.lazyTracked(() => source());
  return <div>{value()}</div>;
}

export function UnsupportedFrames() {
  const [source] = createSignal(1);
  lazyTracked(async () => { await Promise.resolve(); return source(); });
  lazyTracked(function* () { yield source(); });
  lazyTracked(function calc() { return source(); });
  lazyTracked((value = source()) => value);
  return <div />;
}

export function ReturnedCallbackEscape() {
  const [source] = createSignal(1);
  const callback = returnedCalc(() => source());
  const stale = callback();
  return <div>{stale}</div>;
}
