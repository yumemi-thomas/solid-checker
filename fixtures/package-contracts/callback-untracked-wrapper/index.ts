import {
  createEffect,
  createRoot,
  onCleanup,
  runWithOwner,
  untrack,
  type Owner
} from "solid-js";

// Positive: `untrack(fn)` invokes `fn` and returns its value before returning
// itself (1.9.14 `dist/solid.js`: `Listener = null; try { return fn() }`). The
// callback therefore runs before `untrackedWrapper` returns -- `inline` -- even
// though nothing it reads subscribes. This is `solid-js/web`'s own `use`.
export function untrackedWrapper(handle: (value: number) => void): void {
  untrack(() => handle(1));
}

// Positive: `createRoot` runs its callback synchronously under a fresh owner.
// This is `@solid-primitives/rootless`' `createSubRoot`/`createBranch`, whose
// generated contract claimed `deferred` for all four probe modes.
export function rootWrapper(handle: (dispose: () => void) => void): void {
  createRoot(dispose => handle(dispose));
}

// Positive: the clearing wrapper at argument 1, with a non-callable argument 0.
export function ownerWrapper(owner: Owner, handle: () => void): void {
  runWithOwner(owner, () => handle());
}

// Negative: no clearing wrapper, so the tracked claim is untouched. A rule that
// answered "not tracked" for every wrapper would break exactly here.
export function trackedWrapper(handle: () => void): void {
  createEffect(() => handle());
}

// Negative: a genuinely later wrapper keeps `deferred`. `onCleanup` stores the
// callback on the owner and the runtime invokes it at disposal.
export function deferredWrapper(handle: () => void): void {
  onCleanup(() => handle());
}
