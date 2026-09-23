# member-alias-proposals

Pins the generator's side of ADR 0103's 2026-09-23 amendment: an export that is
a `const` alias of a member access has no body to summarize, so no walk clears
its `creates` and its raised summary leaves `callbacks` open. The generator now
proposes both empty closures beside the `reads` it already proposed, for the
certifier's default-library alias census to decide.

The flag is syntax only (`ContractExport::member_alias_initializer`): a `const`
binding of one identifier, found by symbol so `export { viaSpecifier }` reaches
its declaration, whose initializer is exactly a non-computed member access.

| export | proposes `creates`/`callbacks` | why |
| --- | --- | --- |
| `direct` | yes | `export const direct = Object.keys` |
| `viaSpecifier` | yes | declared once, exported by name |
| `ownMember` | yes | syntax cannot tell a package's own object from a default-library one; the certifier refuses it |
| `reassignable` | no | a `let` |
| `computed` | no | `table["run"]`, a computed member |
| `bound` | no | a call initializer |

A destructuring pattern (`export const { is } = Object`) would be the fourth
negative, and the code refuses it, but it cannot be one here: the resolver finds
no exact runtime binding for a destructured export and refuses the whole case
before the generator runs.

What the certifier does with the proposals is pinned by
`a_reviewed_default_library_alias_closes_by_identity`, over
`../value-exports`' `entries`. No `solid-js` stub: nothing here imports it.
