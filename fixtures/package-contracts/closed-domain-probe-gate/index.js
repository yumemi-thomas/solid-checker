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

// Two exports TypeScript cannot tell apart, with opposite reactive-ownership
// behavior. `run` creates no reactive owner; `runCreatingOwner` does. That
// difference is real and completely absent from `index.d.ts`, which is why a
// `creates: []` proposal about either is refused: the declaration census that
// discharges `DomainExhaustiveness` is identical for the two, so admitting it
// would leave a probe's finite non-observation as the only discriminator.
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
