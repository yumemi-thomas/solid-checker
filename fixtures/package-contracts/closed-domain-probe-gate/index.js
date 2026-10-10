// The runtime the declarations describe, and in one place deliberately do not.
//
// `entry` is one of the two alternatives `index.d.ts` declares — a callable.
// `driftedEntry` is neither: the package ships a number while promising
// `(() => void) | undefined`. `tsc` reports nothing about either — a consumer
// reads both as that union, which is what the declarations say — so the drift
// is a publisher defect only a runtime observation can catch. That is the whole
// job of a probe gate: it cannot establish the closed claim, and it can refuse
// a proposal the package's own runtime contradicts.
export const entry = () => {};
export const driftedEntry = 42;

// Two exports TypeScript cannot tell apart — and, to a `creates` census,
// indistinguishable in what they *do*, because neither performs a `create`
// operation. `runCreatingOwner` parks an object literal in a module-level
// variable; under `semantic-model.md` § creates that is neither a version-1
// resource nor a registration into a runtime that acts on it (nothing in Solid
// consults `currentOwner`), so it is not a `create`. The only call in either
// body is `callback()`, a callee rooted at a parameter, whose body is the
// caller's behavior. `run` therefore certifies `creates: []` through the
// implementation census (`docs/adr/0008-implementation-census-for-creates.md`).
// So does `runCreatingOwner`, and for the same disposition — but only since the
// producer began classifying its control-flow markers. Its `try … finally` puts
// a `tryReachability` marker in the control-flow census, and the census used to
// refuse *every* marker, because the producer withheld call rows inside regions
// a `break`/`continue` makes non-universal and a marker was the only trace such
// a row left. A `try` withholds nothing itself, so that was an over-refusal
// (ADR 0008 item 0). The marker is now classified `reachability-lower-bound` —
// the construct is walked in full and every call inside it is on the wire — and
// the census admits it and disposes the same one call. This pair is the pin for
// that item being discharged: nothing about either body changed.
//
// The declaration census alone could never have decided either export, which is
// why the domain was refused by name before the implementation census existed.
let currentOwner = null;

export function run(callback) {
  return callback();
}

export function runCreatingOwner(callback) {
  const owner = { disposals: [] };
  const previous = currentOwner;
  currentOwner = owner;
  try {
    return callback();
  } finally {
    currentOwner = previous;
  }
}
