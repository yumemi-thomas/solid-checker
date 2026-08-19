# v1/reactive-dispatch-unresolved

`SC9012` · **warning** · uncertifiable

A type-correct call can reach runtime implementations with different or
unknown reactivity or ownership behavior, or an exact synchronous callback
position receives a body the checker cannot inspect. The call therefore cannot
be certified as safe or reported as a proven violation.

## What it does

Reports a proof obligation for unresolved runtime dispatch: a conditional or
union object supplied to a helper that invokes `argument.method()`, a computed
call target, or an exported structural helper that directly dispatches outside
a compiler-proven tracked JSX region and has unseen callers. Resolved
standard-library methods are not open dispatch.

It also covers a leaf-owner primitive receiving an opaque callback factory or
wrapper. Exact in-project callback references are inspected, so a safe body is
certified and a forbidden operation retains its specific SC3xxx rule; SC9012
is only the unresolved branch.

An exact built-in synchronous callback position after tracking has ended, such
as `Array.prototype.filter` after `await`, also remains SC9012 when its callback
body is hidden behind a wrapper or is async.

An exported structured return also remains SC9012 when a shorthand value comes
through an ambiguous, bare/path-mapped, or global binding that cannot be joined
exactly.

When every exact candidate is available and their reactive-read summaries are
equivalent, the common summary is used. Missing or divergent candidates remain
explicitly uncertifiable rather than disappearing from the Solid 1.x analysis.

The call must be valid according to TypeScript facts. TypeScript-invalid calls
remain TypeScript-owned; SC9012 describes only the reactivity property that the
type system cannot express. Its warning severity does not bypass `--certify`.

## How to fix

Narrow the runtime value to one exact implementation, or wrap the alternatives
in an analyzed adapter with one explicit reactive behavior. Do not expose a
structural parameter-member dispatch from a public helper when its contract
must be certifiable.

## Related

- [v1/reactive-source-uncaptured](reactive-source-uncaptured.md) — a reactive value crosses an undescribed package call
- [v1/strict-read-untracked](strict-read-untracked.md) — a reactive read is proven to execute untracked
