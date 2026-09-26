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
// `dist/dev.js:4380`), and nothing in an untyped call says which runtime it
// will meet, so `withoutKeys` now leaves `callbacks` open. With two or more key
// arguments no predicate form exists, so the suppression this fixture pins
// still closes the domain here.
export function withoutEitherKey(props, first, second) {
  return omit(props, first, second);
}
