import { createEffect, createMemo } from "solid-js";

// Positive: the `@solid-primitives/spring` `createDerivedSpring` /
// `@tanstack/solid-pacer` `createDebouncedValue` shape -- one invocation in the
// export's own body and one inside a tracked compute. One row is pushed per
// invocation site, so this parameter carried an `inline` row *and* a `tracked`
// row: two mutually exclusive claims about one runtime behavior, at most one of
// which a probe can ever confirm.
export function inlineAndTracked(handle: () => void): void {
  handle();
  createEffect(() => handle());
}

// Positive: the `@solid-primitives/range` `mapRange` shape -- an inline site in
// the body and a site inside the accessor the export hands back. That pair is
// the one the corpus measurement caught directly, as `callbacks[2]=deferred`
// and `callbacks[2]=tracked` on the same summary.
export function inlineAndReturned(step: () => number): () => number {
  const first = step();
  return () => first + step();
}

// Negative: two invocation sites with the *same* schedule are not a
// contradiction. Identical rows dedup, so this publishes one `tracked` row --
// a rule keyed on "more than one row for a parameter" would wrongly open the
// sentinel here.
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

// Positive, and the one that pins how *wide* the sentinel is: parameter 0
// contradicts itself, parameter 1 carries one undisputed `tracked` row. The
// whole `callbacks` domain goes unknown, parameter 1's proof included.
//
// That is deliberate, and it is not a narrowing schema v1 can express. The only
// sub-domain granularity below `{"status":"unknown"}` is a row's presence, and
// an absent row is a *certified negative* -- "this export never invokes a
// caller-supplied callback at that parameter" (docs/package-contracts.md, the
// "no callback execution row" review section). So dropping only parameter 0's
// rows would trade one contradiction for one affirmative false negative, which
// is worse. Keeping parameter 1's row while stating nothing about parameter 0
// is not expressible at all. The per-export sentinel is the honest encoding,
// and it is the same width the pre-existing `escaped_parameters` sentinel has.
export function contradictOnZeroOnly(a: () => void, b: () => void): void {
  a();
  createEffect(() => a());
  createEffect(() => b());
}

// Negative: the single-site control, so the sentinel cannot be blamed on
// `createEffect` or on the export shape.
export function oneTrackedSite(handle: () => void): void {
  createEffect(() => handle());
}

// Negative: an inline site alone.
export function oneInlineSite(handle: () => number): () => number {
  const value = handle();
  return createMemo(() => value);
}
