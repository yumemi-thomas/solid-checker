# implementation-census-described-accessor

The tracer for item A of `docs/package-contract-v2/phase22/2026-09-24-ways-to-improve.md`
§ 3.3: a `callbacks` item for a **non-call** use of the caller's argument. An
`invoke` operation may state a `protocol` -- `get` (a property read, which may
run a getter or a proxy trap), `iterate`, `coerce` (ToPrimitive) or
`has-instance` -- and such an item is always at the call event on the same
stack, `ambient-at-execution`, counted per call from zero to many, and named by
exactly one `callbacks` item from a bare parameter.

It is two fixtures in one. As a generator-corpus fixture (`corpus.json`) its
`expected.json` pins what the generator derives from its own walk: one `get`
item per parameter whose own identifier is the object of a member read in the
export's body (not the callee of a call, not in write position, outside any
nested callable), and one `coerce` item per parameter that is itself an
operand of a coercing operator. As a certification tracer
(`the_described_accessor_census_certifies_exactly_the_described_protocols` in
`contract_certification.rs`) it is planned with each export's items set by
hand, including claims the walk would not make, and certified against the real
producer with the synthesized per-protocol Proxy veto.

| export | generator proposes | tracer claims | verdict |
| --- | --- | --- | --- |
| `access` | `call 0`, `get 0` | the same | certified |
| `compare` | `coerce 0`, `coerce 1` | the same | certified |
| `deferredRead` | nothing (the read is in a returned closure) | `get 0` | withheld: no uncaptured read of the argument at the call |
| `nestedRead` | nothing (the read is in a nested callable) | `get 0` | withheld: the same |
| `defaultRead` | nothing (`w` has a default) | `get 1` | withheld: no read rooted at the caller's own value at slot 1 |
| `readAndCoerce` | `get 0`, `coerce 0` | `get 0` only | refused: the coercion of parameter 0 is a member the enumeration does not describe (ADR 0100 rule 2, its own words) |
| `plainArithmetic` | `coerce 0`, `coerce 1` | the same | certified as `callbacks: []`: both items narrow |
| `callAndAdd` | `call 0`, `coerce 1` | the same | certified as `[call 0]`: only the coercion narrows |
| `isNonNullable` | nothing (`i != null` coerces nothing) | the same | certified as `callbacks: []` |
| `destructureAndMeasure` | `get 1`, not proposed (it also iterates `point`) | `get 1` | withheld: no form for `polygon.length`, and `Polygon` admits an object, so the item does not narrow and the domain opens |
| `isPointInPolygon` | `get 1`, not proposed (it iterates `point` and `polygon[i]`) | `get 1` | refused: the census ran under the declared-signature premise for object-typed parameters |

`access` and `compare` are declared exactly as
`@solid-primitives/utils@7.0.0-next.4` declares them, and their bodies are that
package's bytes. `typeof` and `!` are not invoking forms, so `access`'s
`.length` read is the one use beside its call; `compare`'s `<` and `>` coerce
both arguments.

The withheld rows withdraw their item by its own positive facts before the
closure census is reached -- the item's `operation-reachability`,
`argument-binding` and `callable-path` demands find no uncaptured use of the
caller's value by that protocol in the export's own frame -- which opens the
domain; the row still certifies.

## Narrowing, and what the declared-signature premise hides

Under ADR 0038's premise the producer classifies iteration, coercion, `await`
and declared-member reads by the parameter's *declared* type. A `number`
parameter's coercion therefore records no form: a `coerce` item for it finds no
use, and since a primitive cannot carry a trap no caller code can run there, so
the item **narrows** out of the closed enumeration -- recorded as a withheld
operation whose reason starts `narrowed out of a closed callbacks enumeration:`
-- and the census re-confirms what is left. A call item's withdrawal still opens
the domain.

For a type that admits an object the premise is not exact: `const [x, y] =
point` with `point: [number, number]` records no form (the engine's array
iterator is presumed), yet a caller-supplied Proxy of that type runs its traps
there. That is the `@kobalte/utils@2.0.0-alpha.0` `isPointInPolygon` case the
ecosystem census found: `[get 1]` was confirmed from its three `polygon[i]`
sites, and the synthesized Proxy veto contradicted it. So a non-call
enumeration is refused whole when the census ran under a premise for an
object-admitting parameter (or read a helper's frame), an object-admitting
item's missing use opens the domain rather than narrowing, and the generator
declines to propose a non-call enumeration beside an iteration of a caller's
value, since it derives no `iterate` item.
