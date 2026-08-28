# v1/package-contract-incomplete

`SC9005` · **error** · uncertifiable

An external package used by the Solid 1.x analysis has no exact, reviewed
reactivity contract, or its contract omits an imported export or callback
execution fact. The checker reports the missing evidence instead of assuming
that the package is transparent or owner-preserving.

## What it does

The message distinguishes a wholly missing/stale accepted input, a missing or
unselected exact artifact/export, and an external callback whose schedule,
tracking, ownership, or cardinality leaf is open. Package identity, runtime and
declaration bytes, closure, receipt, and proof policy must all match; evidence
for another release or same-named fixture cannot certify it. The diagnostic
contains an open-claim descriptor, never an editable contract stub.

## How to fix

Generate and review a contract for the installed artifact:

```sh
solid-checker contract generate \
  --package-root node_modules/example \
  --integrity 'sha512-…' \
  --output .solid-checker/contracts/example/solid-reactivity.json
```

Review the exact open leaf, prove every required family, issue the receipt with
`solid-checker contract verify`, and register the document/receipt/full import
resolution in `.solid-checker/accepted-contracts.json`. Do not add blanket
trust or copy a Solid 2 claim into this dialect: describe only behavior proved
for the exact Solid 1 artifact case.

## Related

- [v1/reactive-source-uncaptured](./reactive-source-uncaptured.md) — an external call with an incomplete source summary
- [v1/reactive-dispatch-unresolved](./reactive-dispatch-unresolved.md) — unresolved project dispatch
- [Package contracts](../../package-contracts.md) — schema and evidence workflow
