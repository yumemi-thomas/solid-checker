import { omit } from "solid-js";

// An untyped artifact, deliberately: with the declarations erased, nothing
// proves `keys` is not callable, so the generator's callback inventory reaches
// the unknown-contract-callback arm for every argument of this call. A props
// split is the one shape where that arm must not fire -- `omit` only builds
// property views, and its key lists are values -- and the suppression that
// says so is what this fixture pins.
export function withoutKeys(props, keys) {
  return omit(props, keys);
}

// Added 2026-09-26, when rc.9's predicate form made the two-argument call above
// ambiguous: `omit(props, hidden)` with a single function argument calls it on
// every read of the returned view (`@solidjs/signals@2.0.0-rc.9`
// `dist/dev.js:4380`). This package's solid-js is rc.3, whose `omit` never
// invokes an argument, so `withoutKeys` closes `callbacks`; on rc.9 it would
// not. With two or more key arguments no predicate form exists on any
// release, so the suppression closes the domain here either way.
export function withoutEitherKey(props, first, second) {
  return omit(props, first, second);
}
