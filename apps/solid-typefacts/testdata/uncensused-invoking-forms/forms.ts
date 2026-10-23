// Five exports over one wire round trip: a tagged template that no call
// census records, a plain call that produces no marker at all, an export whose
// body calls a module-local helper the export path cannot name, and two whose
// control flow is incomplete in the two different ways the census
// distinguishes.
//
// The Rust process tests ask for all five against the real producer, so they
// pin the CBOR encoding of the marker rows, both closed enums, the
// local-declaration demand's identity binding, and the reach of a call row a
// jump region covers — none of which the Go producer tests exercise, because
// they call the analyzer in process.

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

export function jumpRegionForm(callback: () => void, flag: boolean): void {
  switch (Number(flag)) {
    case 1:
      callback();
      break;
  }
}

export function unaccountedJumpForm(callback: () => void): void {
  outer: {
    callback();
    break outer;
  }
}
