import { createEffect, createMemo } from "solid-js";

// Positive: the `@solid-primitives/spring` `createDerivedSpring` /
// `@tanstack/solid-pacer` `createDebouncedValue` shape -- one invocation in the
// export's own body and one inside a tracked compute. The normalized proposal
// must not collapse these mutually incompatible operation axes into one closed
// callback claim.
export function inlineAndTracked(handle: () => void): void {
  handle();
  createEffect(() => handle());
}

// Positive: the `@solid-primitives/range` `mapRange` shape -- an inline site in
// the body and a site inside the accessor the export hands back. That pair is
// the one the original corpus measurement caught directly. Temporary-v2 keeps
// the callback leaf open instead of serializing two contradictory rows.
export function inlineAndReturned(step: () => number): () => number {
  const first = step();
  return () => first + step();
}

// Negative: two invocation sites with the *same* schedule are not a
// contradiction. Equivalent operations deduplicate, so this proposes one
// queued/tracked callback operation.
export function twoTrackedSites(handle: () => void): void {
  createEffect(() => handle());
  createEffect(() => handle());
}

// Negative: two *parameters* with different schedules. The axis is per
// parameter, so nothing here is contradictory and both rows stand.
export function twoParameters(now: () => void, later: () => void): void {
  now();
  createEffect(() => later());
}

// Positive locality case: parameter 0 contradicts itself while parameter 1 has
// one undisputed tracked site. The proposal stays open at the exact callback
// domain rather than turning the unresolved parameter into negative proof.
export function contradictOnZeroOnly(a: () => void, b: () => void): void {
  a();
  createEffect(() => a());
  createEffect(() => b());
}

// Negative: the single-site control, so openness cannot be blamed on
// `createEffect` or on the export shape.
export function oneTrackedSite(handle: () => void): void {
  createEffect(() => handle());
}

// Negative: an inline site alone.
export function oneInlineSite(handle: () => number): () => number {
  const value = handle();
  return createMemo(() => value);
}
