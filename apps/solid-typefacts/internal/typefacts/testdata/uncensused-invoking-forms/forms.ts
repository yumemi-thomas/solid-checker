// Three exports over one wire round trip: a tagged template that no call
// census records, a plain call that produces no marker at all, and an export
// whose body calls a module-local helper the export path cannot name.
//
// The Rust process test asks for all three against the real producer, so it
// pins the CBOR encoding of the marker rows, the closed kind enum, and the
// local-declaration demand's identity binding — none of which the Go producer
// tests exercise, because they call the analyzer in process.

export function taggedForm(tag: (parts: TemplateStringsArray) => string): string {
  return tag`plain`;
}

export function plainCallForm(callback: () => void): void {
  callback();
}

function localHelper(callback: () => void): void {
  callback();
}

export function entry(callback: () => void): void {
  localHelper(callback);
}
