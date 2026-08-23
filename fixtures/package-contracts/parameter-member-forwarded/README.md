# The parameter-member row discharges the obligation only where it is published

An exported helper that invokes a member of one of its own parameters
(`client.getThing()`) has callers outside the analyzed project, so project
analysis keeps the obligation explicit. Contract emission may discharge it,
because `contract_export_function` serializes the same provenance as a
`parameter-member` reactive read and a consumer resolves that row against the
argument it actually passes — that is the discharge, and `./direct` is it:
`channelFor` publishes `reactiveReads: [{ parameter-member, parameter 0 }]` and
carries no unknown claim.

Emission used to discharge the obligation for *every* export by comparing
`analysis_context` to a string, before asking who the obligation belonged to.
The provenance does not survive a hop. `forwarded` calls
`channelFor(props.client)`: the receiver is a member of `forwarded`'s parameter,
not a parameter of `forwarded`, so nothing re-establishes the provenance and
`forwarded` publishes **no row at all**. A consumer calling `forwarded` was told
the export reads nothing reactive — a certified negative about behavior that
depends entirely on what it is handed.

So the discharge is asked of the exports the attribution ladder actually
resolves the obligation to, and holds only when every one of them publishes the
row. `.` fails that test and `forwarded` is marked; `./direct` passes it and
`channelFor` keeps its exact row.

`Isolated` reaches nothing and stays certified.

`channel.js` deliberately has no sibling `channel.d.ts`. With one, every
importer binds to the declaration file instead, the caller edge from `index.js`
is lost, and the enumeration reports itself incomplete so attribution widens to
every export of the entrypoint. That is a different claim — this fixture pins
the *exact* attribution, and `declaration-sibling-reach` pins the widening.
