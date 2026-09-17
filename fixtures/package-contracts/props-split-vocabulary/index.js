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
