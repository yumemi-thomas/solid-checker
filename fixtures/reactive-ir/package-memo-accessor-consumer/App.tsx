import { createDoubled, createDoubledOpen } from "reactive-package";
import { createSignal } from "solid-js";

// `createDoubled`'s contract closes `returns` over a described callable that
// reads a memo the package created (ADR 0162), so what it returns is an
// accessor. Calling it in the component body reads it outside any tracking
// scope: the untracked read is reported where it happens.
export function UntrackedDoubled() {
  const [count] = createSignal(1);
  const doubled = createDoubled(count);
  const value = doubled();
  return <div>{value}</div>;
}

// The correct twin: the same accessor read inside JSX is tracked, and nothing
// is reported.
export function TrackedDoubled() {
  const [count] = createSignal(1);
  const doubled = createDoubled(count);
  return <div>{doubled()}</div>;
}

// The control: the same declaration and the same use, with `returns` left
// open, so the import reports the open claim.
export function DoubledOpen() {
  const [count] = createSignal(1);
  const doubled = createDoubledOpen(count);
  const value = doubled();
  return <div>{value}</div>;
}
