# open-owner-requirements

ADR 0174: an open owner-requirement list still publishes the guaranteed items
proven beside it.

`partial-contract-package` and `.solid-checker/accepted-contracts.json` are
`../unresolved-contract-export-attribution/`'s, byte for byte. The catalog
accepts the dependency's policy-1 contract, which this build refuses
(`obsolete-policy1-receipt`), so the import raises a
`PackageContractExportMissing` obligation at the import line. No function
contains it, and it is attributed to every export (`FallbackAll`), opening
all five claim domains, owner requirements included. The `solid-js` stub is
`../host-constant-browser/`'s, whose `onCleanup` is byte-faithful to
`@solidjs/signals@2.0.0-rc.9`.

- `cleanupThenUndescribed` registers a cleanup on every call. The open list
  still publishes that item with `min: 1`. `creates` stays unknown: the list
  is not complete, and a closed `creates` would tell a consumer it is.
- `conditionalCleanupThenUndescribed` registers the cleanup on some calls
  only. Under an open list that possible item is not published.
- `registerCleanup` and `addCleanup` are one function under two names. The
  runtime-alias merge used to rebuild them from an empty summary, so the
  cleanup was lost before the obligation opened the list. The merge now
  carries the union, and both names keep the `min: 1` item.

The list stays open throughout. An item a later census proves or withdraws
by name is what this fixture pins; it never closes the domain.
