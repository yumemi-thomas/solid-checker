# ADR 0259: createRoot passes its callback's return value through

- Status: accepted and implemented (2026-10-09).
- Owners: `Dialect::returned_callback_value` (Solid 2 answers slot 0 for
  `createRoot`); `callback_return.rs`; the callback-result branch of source
  discovery; `SemanticLookup::contract_return_at_call`.

Solid 2's `createRoot(fn)` returns `fn`'s value. Until now the checker gave a
binding of that value no identity, so `const now = createRoot(() =>
createPolled(...))` followed by a component-body `now()` was never a strict
read. ADR 0257 made the ledger twins build primitives this way, which exposed
the gap.

The runtime passthrough is a dialect fact; the proof is shared. A binding gets
the inner factory's exact identity only when all of these hold:

- the callback is one synchronous, unnamed, parameterless literal (arrow or
  function expression) with exactly one complete return;
- that return is an exact dialect-known or contracted factory call in the
  callback, or an immutable local initialized by one and used only there;
- the outer binding is immutable, not exported, not escaped, and every use is
  a call; destructuring selects through the factory's tuple or object shape.

A resolved binding takes its identity from this proof, not from type
annotations. Anything else is left unresolved and keeps exactly the evidence
it had before.

## Rejected: an obligation on every unresolved root result

The draft made every used, unresolved `createRoot` result a
`reactive-dispatch-unresolved` obligation. Over the rc.13 corpus that added 123
uncertifiable sites. Typical cases were roots returning their own `dispose`,
async test roots and plain objects. It flipped no ledger case, so it was
dropped. Unresolved results are therefore no worse than before, and no better.

## Measured consequences

- Fixture `root-callback-return`: nine shapes report SC1001 violations at
  component-body reads. Tracked JSX reads and every unresolved shape add no
  finding.
- Primitives ledger, browser: `createPolled` and `createPagination`
  top-level-read misuses are now proven violations ("wrong finding" to
  "correct use not clean"). Both correct twins still carry the specs' own
  gaps: `createPolled` callbacks, and `createPagination` reactive reads and
  owner requirements. `createIntervalCounter` is unchanged. No correct twin has
  a violation.
- rc.13 corpus: no violation or uncertifiable site added or removed.
