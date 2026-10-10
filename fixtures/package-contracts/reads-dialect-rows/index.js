// ADR 0168: the `reads` census's call walk and the argument-scoped dialect
// rows. Each export is censused with `reads: []` proposed by the
// `the_reads_walk_admits_*` test in
// `rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`
// over transcripts synthesized from these spans; README.md says what each must
// come to.
import { createSignal, getOwner, untrack } from "solid-js";

// The literal is walked with the frame; `sig` is the caller's own accessor.
export function untrackLiteral(sig) {
  return untrack(() => sig());
}

// The caller's own callable, by reference: the caller's code either way.
export function untrackParameter(read) {
  return untrack(read);
}

// A function first argument: `solid-js` sends it to the hydration gate.
export function createSignalOfFunction(sig) {
  return createSignal(() => sig());
}

// A primitive first argument: a plain signal, never the gate.
export function createSignalOfPrimitive() {
  return createSignal(0);
}

// Reads nothing whatever its arguments.
export function ownerOnly() {
  return getOwner();
}
