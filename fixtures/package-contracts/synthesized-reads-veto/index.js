// The subject of ADR 0163's synthesized `reads: []` veto, run against the real
// `@solidjs/signals` bytes by `synthesized_vetoes_tests.rs`. Every export is
// proposed `reads: []` by the test, true or not: the census would refuse most
// of these, and the point here is what the *veto* sees on its own.
import { createMemo, createSignal, untrack } from "@solidjs/signals";

// A source this module owns.
const [owned] = createSignal(1);

// --- The veto must stay quiet: `reads: []` is true. ---

// Arithmetic on its argument; reads nothing.
export function readsNothing(step) {
  return step + 1;
}

// --- The veto must contradict: each call reads a source. ---

// A signal this module owns, read at the call.
export function readsOwnSignal(step) {
  return owned() + step;
}

// A signal the call itself creates, then reads.
export function readsCreatedSignal(step) {
  const [created] = createSignal(step);
  return created();
}

// A memo the call creates, then reads: a read of a created source.
export function readsCreatedMemo(step) {
  const doubled = createMemo(() => step * 2);
  return doubled();
}

// A read, and then a throw on every call: the read is still observed.
export function readsThenThrows(step) {
  owned();
  throw new Error(`rejected ${step}`);
}

// --- Incomplete: nothing completes and nothing reads. ---

export function throwsWithoutReading(step) {
  throw new Error(`rejected ${step}`);
}

// --- Reads the veto does not observe, by its stated limitation. ---

// An untracked read links no dependency.
export function readsUntracked(step) {
  return untrack(owned) + step;
}

// A read the call defers past its own stack.
export function readsLater(step) {
  queueMicrotask(() => owned());
  return step;
}

// A read performed by a computation the call creates, and never read back:
// the dependency is that memo's, not the observing one's.
export function createsAReadingMemo(step) {
  createMemo(() => owned() + step);
  return step;
}
