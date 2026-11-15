# total-decision-silences

**Claim (ADR 0987, slice 8a).** SC5006 (`loading-on-outside-hold`) and SC5007 (`loading-on-accessor`) decide every candidate of one site universe, the `loading-on` table (`proof_universe.rs`): each place the dialect's async boundary is entered with a way to pass `on` (a tag, a namespace member, an exact call, the primitive behind it, a value reference, an unresolved tag), found by identity and never spelling. A boundary with no `on` and no spread is a filter, not a site. Each negative exit of either rule is a typed silence with its premise, never a bare return, so an unnamed exit is an uncertifiable row (blockers `loading-on-outside-hold-unaccounted`, `loading-on-accessor-unaccounted`), never a certified clean:

| silence | code | premise (kind, span) |
|---|---|---|
| `on-names-nothing` | LOADING_ON_OUTSIDE_HOLD | `OptionLiteral`: the value of `on` (no identifier, no call: no dependency to change) |
| `holds-no-async-source` | LOADING_ON_OUTSIDE_HOLD | `BoundaryHoldsNoAsync`: the children, only where `unproven_hold` answers `None` |
| `on-not-function` | LOADING_ON_ACCESSOR | `OnValueNotCallable`: the value of `on` (the compiler's callability is non-callable) |

Both codes are placeless, so an SC9012 never credits them. `NamesNothing`, `HoldsNothing`, `Namespace` and `HandCall` each end in the silences above and produce no finding; `Shadowed` (a local `Loading`) is no site; `SyncOptimistic` (a `createOptimistic(0)` setter in a handler) carries no SC2009 code, as its source is synchronous. With `SOLID_CHECKER_IR_ABLATE_SILENCE=all` every site of the first four becomes an uncertifiable row of its rule: the process test `a_loading_boundary_exit_is_a_named_silence_and_unnamed_it_is_an_uncertifiable_row` pins it.

The fixture's snapshot also holds rows of other families (SC3001, SC4001, SC6002 at the component tags, SC9014 for the stub's package): they are those universes' own and are not part of this claim.
