# Package names do not authorize external semantics

Neither import in this fixture has an exact, receipt-issued accepted-contract
entry. `reactive-package` resolves through `paths` to project source;
`uninstalled-package` is supplied only by an ambient declaration. Both remain
outside the package-contract trust boundary even though their spellings look
like package names.

Phase 14 deleted the former project-local schema-v1 contracts from this
fixture. Their old behavior—allowing an unresolved specifier to bind by name—
cannot be expressed at the normalized consumer boundary. Acceptance now needs
an exact host resolution, matching artifacts and closure, and a proof receipt;
missing resolution evidence never becomes permission to apply a claim.

`tsc --noEmit` is silent. The fixture is a provenance control, not a duplicate
type diagnostic.
