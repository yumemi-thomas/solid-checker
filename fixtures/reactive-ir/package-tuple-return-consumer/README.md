# A consumer reading through a wrapper into a reactive tuple

Pins `Dialect::returns_reactive_tuple` — which Solid 2.0 primitives return a
two-slot `[source, setter]` tuple rather than the source itself — at the
**behaviour** level. Until this fixture the row was correct-by-construction:
pinned by `each_dialect_names_its_own_props_split_and_tuple_returns` against the
audited declarations, and by nothing that ran the checker.

## The premise, and why it needs an accepted contract

`effective_inner_call_return` is the row's only reader, and it is reached only
from `effective_call_return`, which does work only when a **contracted** export's
own `returns` is `argument` or `callback-result`. So the fixture needs a package
whose accepted contract says "I return argument 0" — `identity` — and reads that
travel through it into a primitive call.

No fixture could supply an accepted contract until 2026-09-15; see
`docs/package-contract-v2/phase21/2026-09-15-an-accepted-contract-a-fixture-can-supply.md`.
This ships `.solid-checker/authorize-contract.json` and coverage authorizes it.

## What each component claims

| component | binding | with the row | without it |
| --- | --- | --- | --- |
| `SignalSlot` | `const [value, set] = …` | SC1001 on `value()` | same |
| `ElidedSlot` | `const [, setOnly] = …` | clean | **SC1001 on `setOnly(1)`** |
| `StoreSlot` | `const [store, set] = …` | SC1001 on `store.count` | same |
| `WholeTuple` | `const whole = …` | clean | **SC1001 on `whole[0].count`** |
| `Projection` | `const projected = …` | SC1001 on `projected.count` | same |
| `Writes` | — | clean | clean |

Two rows move, and both move the wrong way without the row: a violation is
*invented* on correct code that `tsc` accepts.

- **`ElidedSlot`.** An unstructured reactive return is attributed to the
  binding's first name. With the first slot elided that name is the **setter**,
  so a bare accessor makes `setOnly(1)` an untracked read of a reactive accessor.
- **`WholeTuple`.** The binding is not a destructure, so a tuple has no slot to
  attribute and nothing is reported. A bare store path instead makes the *tuple*
  the store, and the message names `whole.count` — a path that does not exist.

`SignalSlot` and `StoreSlot` are here to say what the defect could **not**
reach: for the ordinary destructure the two shapes agree on slot 0, which is
exactly why a list belonging to neither dialect survived the seam extraction
with every gate green.

`Projection` pins the row's deliberate *absence*: `createProjection` returns the
store itself (`Refreshable<Store<T>>`), so its read is a store path. Adding it
to the tuple list would silence this one — the falsifier in the other direction.

## Stubs

`solid-js.d.ts` quotes the audited return shapes; `Refreshable` is dropped from
`createProjection` because no claim here reads `.refresh`, and that is the only
place these stubs are narrower than the package. Nothing is looser.

`node_modules/reactive-package/package.json` is byte-identical to
`package-return-consumer`'s, so the closure digest and integrity in the import
block are ones this repository already pins rather than numbers invented here.
`node_modules/solid-js` selects the 2.0 catalog and is required: an authorized
fixture is analyzed from a copy, which cannot inherit a dialect from ancestors.
