import {
  createEffect,
  createRoot,
  onCleanup,
  runWithOwner,
  untrack,
  type Owner
} from "solid-js";

// Positive: `untrack(fn)` invokes `fn` and returns its value before returning
// itself (`@solidjs/signals@2.0.0-rc.3` clears the listener, calls, restores).
// The callback therefore runs before `untrackedWrapper` returns -- `inline` --
// even though nothing it reads subscribes.
export function untrackedWrapper(handle: (value: number) => void): void {
  untrack(() => handle(1));
}

// Positive: `createRoot` runs its callback synchronously under a fresh owner.
// 2.0 keeps the `dispose`-taking arm of the init union, so this is the same
// shape `@solid-primitives/rootless`' `createSubRoot` publishes.
export function rootWrapper(handle: (dispose: () => void) => void): void {
  createRoot(dispose => handle(dispose));
}

// Positive: the clearing wrapper at argument 1, with a non-callable argument 0.
// `runWithOwner(owner, fn)` keeps that parameter order in 2.0.
export function ownerWrapper(owner: Owner, handle: () => void): void {
  runWithOwner(owner, () => handle());
}

// Negative: no clearing wrapper, so the tracked claim is untouched. A rule that
// answered "not tracked" for every wrapper would break exactly here.
//
// This is the one export the 1.x original could not be transcribed verbatim.
// 2.0 splits `createEffect` into a tracked *compute* and an untracked effect
// function; the 1.x spelling `createEffect(compute)` survives only as a
// deprecated overload returning `never`
// (`@solidjs/signals dist/types/signals.d.ts:378`).
//
// Checked rather than assumed: in **statement position that overload is not a
// type error**, against the published typings or this stub -- discarding a
// `never` is fine, so `tsc --noEmit` is silent on it and only a use of the
// result would fail. The two-argument form is therefore used because it is the
// supported one, not because the alternative fails to compile. `handle` stays
// in the compute, which is the tracked, deferred position the original
// claimed; the second argument is present because the signature requires it,
// and is empty so nothing but the compute can account for the claim.
export function trackedWrapper(handle: () => void): void {
  createEffect(
    () => handle(),
    () => {}
  );
}

// Negative: a genuinely later wrapper keeps `deferred`. `onCleanup` stores the
// callback on the owner and the runtime invokes it at disposal.
export function deferredWrapper(handle: () => void): void {
  onCleanup(() => handle());
}
