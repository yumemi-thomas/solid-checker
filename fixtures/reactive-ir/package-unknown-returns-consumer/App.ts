import { openPlain, openSource } from "partial-returns-package";

// The reviewed contract marks `returns` -- not `callbacks` -- unknown for
// `openSource`. Nothing at this call site can supply the missing claim, so
// the obligation is opened where the claim enters the project: the import
// binding. Whether the returned object is an accessor, a store path, or
// snapshot data is exactly what the contract declines to state.
export function readSource() {
  const source = openSource();
  return source.value;
}

// The sibling export's summary states every claim it could state: no reactive
// reads, no callbacks, no owner requirement, no async behavior, and a return
// that certifies as non-reactive by its absence. Nothing is unknown, so the
// same shape of use stays clean.
export function readPlain() {
  const plain = openPlain();
  return plain.value;
}
