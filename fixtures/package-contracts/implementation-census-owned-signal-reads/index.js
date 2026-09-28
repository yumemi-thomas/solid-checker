import { createSignal } from "solid-js";

// The tracer for ADR 0146's generator half: an export whose reactive analysis
// describes the return as an accessor, and whose every value-carrying
// completion is the accessor itself or a literal, proposes a described callable
// that reads a signal it owns.

// The accessor itself: invoking it reads the signal and hands back its value.
export function createCounter() {
  const [count] = createSignal(0);
  return count;
}

// A literal that reads it and hands back what it read.
export function createReader() {
  const [count] = createSignal(0);
  return () => count();
}

// A literal that reads it and hands back a number by grammar.
export function createDoubled() {
  const [count] = createSignal(1);
  return () => count() * 2;
}

// The signal is created over the caller's argument, which may be a function
// (a writable memo, whose read runs it). Proposed the same way; the census
// refuses the read by name.
export function createFrom(initial) {
  const [value] = createSignal(initial);
  return value;
}
