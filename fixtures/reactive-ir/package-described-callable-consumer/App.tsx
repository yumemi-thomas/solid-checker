import { createCounter, createIdGenerator, createIdGeneratorOpen } from "reactive-package";

// `createIdGenerator`'s accepted contract closes `returns` over one described
// callable that reads nothing (ADR 0145). The import finds nothing open, and
// calling what it returns -- in the component body, untracked -- reads
// nothing reactive, so nothing is reported.
export function Ids() {
  const next = createIdGenerator();
  const id = next();
  return <div>{id}</div>;
}

// The control: the same declaration and the same use, with `returns` left
// open, so the import reports the open claim. The closure above has to remove
// exactly this finding.
export function IdsOpen() {
  const next = createIdGeneratorOpen();
  const id = next();
  return <div>{id}</div>;
}

// `createCounter`'s contract closes `returns` over a described callable that
// reads a signal the package created (ADR 0146), so what it returns is an
// accessor. Calling it in the component body reads it outside any tracking
// scope: the untracked read is reported where it happens.
export function UntrackedCount() {
  const count = createCounter();
  const value = count();
  return <div>{value}</div>;
}

// The same accessor read inside JSX is tracked, and nothing is reported.
export function TrackedCount() {
  const count = createCounter();
  return <div>{count()}</div>;
}
