// ADR 0165: the `reads` census walks calls. Each export below is planned with
// `reads: []` proposed and certified by `contract_certification.rs`'s
// `reads_call_walk_*` tests; what each must come to is in README.md.

// A value this module builds and hands back as an accessor. Nothing here is
// reactive, and that is the point: the census cannot tell an accessor of a
// plain box from an accessor of a signal the call created, so calling what a
// call returned refuses either way.
function makeAccessor(value) {
  const box = { value };
  return () => box.value;
}

function double(value) {
  return value * 2;
}

function readsThrough(value) {
  const read = makeAccessor(value);
  return read();
}

function even(n) {
  return n === 0 ? true : odd(n - 1);
}

function odd(n) {
  return n === 0 ? false : even(n - 1);
}

function evenReading(n, value) {
  if (n === 0) {
    const read = makeAccessor(value);
    return read();
  }
  return oddReading(n - 1, value);
}

function oddReading(n, value) {
  return evenReading(n - 1, value);
}

// `createCountdown`'s shape: the call builds an accessor and calls it.
export function readsCreatedAccessor(value) {
  const read = makeAccessor(value);
  return read();
}

// The same read one frame down, in a same-package helper.
export function callsReadingHelper(value) {
  return readsThrough(value);
}

// No call at all: the form census alone decides it, and the walk has nothing
// to disposition.
export function callsNothing(value) {
  return value + 1;
}

// A same-package helper that reads nothing.
export function callsPureHelper(value) {
  return double(value);
}

// The caller's own accessor, invoked: the `callbacks` domain's item.
export function invokesArgument(read) {
  return read();
}

// A standard-library member by identity (ADR 0149).
export function callsStandardLibrary(value) {
  return Math.max(value, 0);
}

// The accessor is returned, not called: ADR 0146's business.
export function returnsReader(value) {
  const read = makeAccessor(value);
  return () => read();
}

// A cycle of helpers that read nothing: the back edge closes.
export function mutualRecursion(n) {
  return even(n);
}

// A cycle one of whose frames reads: the back edge does not excuse it.
export function readingCycle(n, value) {
  return evenReading(n, value);
}
