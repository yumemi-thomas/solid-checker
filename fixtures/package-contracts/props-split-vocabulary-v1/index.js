import { splitProps } from "solid-js";

// The 1.x half of the pair. Byte-for-byte the same shape as the 2.0 fixture
// beside it, under the other dialect's spelling of the same primitive -- which
// is the whole point: the suppression used to be written as a comparison
// against 1.x's `Primitive::SplitProps`, so this half always worked and the
// 2.0 half never did.
export function withoutKeys(props, keys) {
  return splitProps(props, keys);
}
