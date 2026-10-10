// The declarations a consumer compiles against, and the only thing that
// decides this fixture's closed claim.
//
// `entry` and `driftedEntry` have byte-identical declared types: a union of
// exactly two alternatives, one callable and one `undefined`. The
// Type Facts census closes `ChoiceAlternatives` for both of them, because the
// census is a census of *this file*: the producer enumerates two alternatives
// and observes both exhaustively, and the verifier requires the proposal's
// enumeration to be that one.
//
// Their runtimes disagree — `driftedEntry` ships a number, which the
// declaration excludes — and that is a publisher defect the type system cannot
// see and a mandatory probe veto can.
//
// `run` and `runCreatingOwner` also have byte-identical declared types. Nothing
// here says what either does with its callback, which is exactly why a
// `creates: []` claim about either is decided by a census of `index.js` — the
// implementation census, which finds one parameter-rooted call in each and
// closes the domain for both — and never by this file or by a probe.
export declare const entry: (() => void) | undefined;
export declare const driftedEntry: (() => void) | undefined;
export declare function run(callback: () => void): void;
export declare function runCreatingOwner(callback: () => void): void;
