import { createSignal as aliasedSignal, createMemo as aliasedMemo } from "solid-js";
import { createDeferredPromise } from "@solidjs/fake-deferred";

// The package's object has a method named `resolve` whose body calls an
// opaque local (its own `let resolve`).
export const deferred = createDeferredPromise<number>();

// V: `Promise.resolve()` is the global built-in under the Promise premise
// (ADR 0349), so its `.resolve` read runs no getter of that name and the
// package's method is not a candidate: nothing before the await reads `read`.
export function Input() {
  const [read] = aliasedSignal(0);
  const memo = aliasedMemo(async () => {
    await Promise.resolve();
    return read();
  });
  void memo;
  return <div />;
}

// U: a plain `.resolve` read that is no proven built-in call's callee keeps
// the getter guess, and the package's `resolve` may read anything.
export function PlainRead() {
  const [read] = aliasedSignal(0);
  const memo = aliasedMemo(async () => {
    void Promise.resolve;
    await Promise.resolve();
    return read();
  });
  void memo;
  return <div />;
}
