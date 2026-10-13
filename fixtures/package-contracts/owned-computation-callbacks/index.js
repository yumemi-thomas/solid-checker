import { createMemo, createEffect, createRoot, onSettled } from "solid-js";

// ADR 0183: a caller's function handed, unconditionally, to an eager owned
// computation the export's own body creates runs during every call as that
// computation's compute.
export function derive(fn) {
  createMemo(fn);
}

// The memo is created only when `flag` holds: the owner is stated, the lower
// bound is not.
export function deriveMaybe(flag, fn) {
  if (flag) createMemo(fn);
}

// An effect's compute runs during the call; its effect function is queued.
export function watch(compute, effect) {
  createEffect(compute, effect);
}

// `fn` is called on every completion of a compute the export writes itself,
// so it runs under that memo too.
export function deriveWrapped(fn) {
  createMemo(() => fn());
}

// The same in an expression-bodied arrow, as `capitalize` is written.
export const deriveArrow = fn => createMemo(() => {
  const value = fn();
  return value;
});

// `fn` is called only when `flag` holds: no owner is stated for it.
export function deriveWrappedMaybe(flag, fn) {
  createMemo(() => {
    if (flag) fn();
  });
}

// An async compute calls `fn` after its first await: no owner either.
export function deriveWrappedAsync(fn) {
  createMemo(async () => {
    await null;
    return fn();
  });
}

// The memo is created inside a root body, which `createRoot` runs during the
// call: `fn` still runs under the memo, on every call.
export function deriveInRoot(fn) {
  return createRoot(dispose => {
    createMemo(fn);
    return dispose;
  });
}

// `onSettled` runs its callback after the call: the memo's owner is stated,
// but not a lower bound or a run during the call.
export function deriveWhenSettled(fn) {
  onSettled(() => {
    createMemo(fn);
  });
}
