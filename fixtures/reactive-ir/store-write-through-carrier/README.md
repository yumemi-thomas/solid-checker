# store-write-through-carrier

**Claim (ADR 1231).** ADR 1172's census of a handed store's reads (a prop's, or since ADR 1211 a provider literal's key) followed a
read only through an exact `const` alias. `carried_store_reads` also follows it through a local carrier that hands it on unchanged:
a data slot of a closed object literal or an element of an array literal, then that literal through an exact `const` binding (an
identifier, or a rest-free pattern of plain slots), a static member read of the key, or a local function's one straight-line return
at each of its calls (its only uses). The store keeps its own literal, so a write past its root is dropped outside the setter.

Violations (Chrome STORE_WRITE_OUTSIDE_SETTER, `runtime-oracle/store-write-through-carrier`): `HookChild`, `TupleChild`,
`HookMemberChild` (a context source). On main they are uncertifiable (`store-write-unproven`).

Uncertifiable: `KeptChild` (the tuple is handed to a function), `HandedHookChild` (the hook is handed on, not only called),
`DefaultChild` (a pattern slot with a default).

Stub: `store-write-through-prop-view`'s (with `node_modules`).
