import { createEffect, createSignal, runWithOwner } from "solid-js";

const [count] = createSignal(0);

// Positive: a plain IIFE at module scope creates an effect. Nothing owns the
// call, so the effect runs with no owner.
export const moduleScope = (() => {
  createEffect(count, () => {});
  return 1;
})();

// Positive: an IIFE inside a callback that `runWithOwner(null, ...)` runs with
// no owner, whatever calls this function.
export function DetachedIife() {
  runWithOwner(null, () => {
    const value = (() => {
      createEffect(count, () => {});
      return 1;
    })();
    return value;
  });
  return <i />;
}

// Negative: an IIFE in a component body runs under the component's owner.
export function Owned() {
  (() => {
    createEffect(count, () => {});
  })();
  return <i />;
}

// Negative: a generator IIFE returns an iterator and runs none of its body.
export const generated = (function* () {
  createEffect(count, () => {});
  yield 1;
})();

// Uncertifiable (ADR 0516): stepping the iterator in place runs the body here,
// which no call edge follows.
export const stepped = (function* () {
  createEffect(count, () => {});
  yield 1;
})().next();

// Negative (ADR 0516): an iterator a statement discards runs none of the body.
(function* () {
  createEffect(count, () => {});
})();

// Negative: an async IIFE in a component body runs the code before its first
// `await` inside the call, under the component's owner.
export function AsyncOwned() {
  (async () => {
    createEffect(count, () => {});
    await Promise.resolve();
  })();
  return <i />;
}

// Positive, pinned: the code after an async IIFE's first `await` is a
// continuation with no owner. Solid's runtime warns NO_OWNER_EFFECT there.
export const asyncLate = (async () => {
  await Promise.resolve();
  createEffect(count, () => {});
})();

// Negative: an unexported function nothing calls is dormant.
function unused() {
  createEffect(count, () => {});
}
