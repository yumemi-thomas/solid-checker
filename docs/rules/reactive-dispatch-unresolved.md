# reactive-dispatch-unresolved

`SC9012` · **warning** · uncertifiable

A type-correct call can reach runtime implementations with different or
unknown reactivity or ownership behavior, or an exact synchronous callback
position receives a body the checker cannot inspect. The call therefore cannot
be certified as safe or reported as a proven violation.

## What it does

Reports a proof obligation when exact runtime dispatch is required by the
reactivity analysis but cannot be established. Covered forms include:

- a helper that invokes `argument.method()` when the argument is a conditional
  or union of objects whose method summaries differ;
- a computed call such as `handlers[index]()` whose runtime target is not an
  exact symbol; and
- an exported helper that directly invokes a member supplied through a
  parameter outside a proven tracked JSX region, because callers outside the
  analyzed project can supply unseen behavior.
- a leaf-owner API receiving a callback through an opaque factory or wrapper,
  or an exact callback whose synchronous helper chain reaches an unresolved
  call. A specific cleanup/flush/primitive violation is not proven, but the
  leaf callback is not certifiably free of all three either; and
- an exact built-in synchronous callback position after tracking has ended,
  such as `Array.prototype.filter` after `await`, when the callback body is
  hidden behind a wrapper or is async; and
- an exported structured return whose shorthand value depends on an ambiguous,
  bare/path-mapped, or global binding that cannot be joined exactly.

Finite candidate sets are not automatically uncertain. If every exact
candidate is present and has the same reactive-read summary, the checker uses
that common summary. If one candidate is missing or the summaries differ, it
reports SC9012 instead of silently choosing a candidate or dropping the call.

SC9012 is shared with Solid 1.x as
[v1/reactive-dispatch-unresolved](v1/reactive-dispatch-unresolved.md). Its
warning severity means that no runtime defect has been proven; its
`uncertifiable` kind still fails `--certify`.

## TypeScript boundary

The rule requires a call that TypeScript facts mark valid. An invalid call is
TypeScript's diagnostic and does not also receive SC9012. The finding asserts
only an unresolved reactivity property that TypeScript types do not express.

## How to fix

Calls inside a compiler-proven tracked JSX expression do not need this
obligation: whichever implementation runs, its reactive reads execute under
that observer. Resolved standard-library methods are likewise excluded.

Narrow the runtime value to one exact implementation, or introduce an adapter
with an analyzed body and one explicit reactive behavior. For a public helper,
avoid invoking structurally supplied methods in a contract that is meant to be
certifiable, or keep the dispatch behind a package boundary with an audited
reactivity contract.

## Related

- [reactive-source-uncaptured](reactive-source-uncaptured.md) — a reactive value crosses an undescribed package call
- [strict-read-untracked](strict-read-untracked.md) — a reactive read is proven to execute untracked
