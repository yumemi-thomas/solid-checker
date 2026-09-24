# member-alias-proposals

Pins the generator's side of ADR 0103's 2026-09-23 amendment: an export that is
a `const` alias of a member access has no body to summarize, so no walk clears
its `creates` and its raised summary leaves `callbacks` open. The generator now
proposes both empty closures beside the `reads` it already proposed, for the
certifier's default-library alias census to decide.

The flag is syntax only (`ContractExport::member_alias_initializer`): a `const`
binding of one identifier, found by symbol so `export { viaSpecifier }` reaches
its declaration, whose initializer is exactly a non-computed member access.

| export | proposes `creates`/`callbacks` | proposes `returns` | why |
| --- | --- | --- | --- |
| `direct` | yes | one return, an array of `plain` | `export const direct = Object.keys` |
| `viaSpecifier` | yes | no: `Object.values`' row states no return | declared once, exported by name |
| `floor` | yes | one return, `plain` | `export const floor = Math.floor` |
| `ownMember` | yes | no: `helpers.run` names no row | syntax cannot tell a package's own object from a default-library one; the certifier refuses it |
| `reassignable` | no | no | a `let` |
| `computed` | no | no | `table["run"]`, a computed member |
| `bound` | no | no | a call initializer |

The `returns` column is the second 2026-09-24 amendment to ADR 0103: the
proposal reads the initializer's *spelling* against the reviewed table's
stated returns, and decides nothing -- the certifier reads the member from the
producer's identity fact.

A destructuring pattern (`export const { is } = Object`) would be the fourth
negative, and the code refuses it, but it cannot be one here: the resolver finds
no exact runtime binding for a destructured export and refuses the whole case
before the generator runs.

What the certifier does with the proposals is pinned by
`a_reviewed_default_library_alias_closes_by_identity`, over
`../value-exports`' `entries`, and by
`a_reviewed_default_library_alias_closes_returns_over_its_reviewed_row` over
this fixture. No `solid-js` stub: nothing here imports it.
