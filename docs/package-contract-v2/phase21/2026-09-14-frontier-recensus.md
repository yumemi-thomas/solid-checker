# Re-censusing the 1,470, with the fixed harness (2026-09-14)

Begun immediately after `ddf43139` repaired the two-pass census. Every verdict
here was produced with a configured corpus, so synthesis ran and the census was
consulted; the numbers the broken instrument produced are not reused.

## The method, and the mistake it is built to prevent

A pass-2 census reports on every proposable candidate on the case. Most of
those are **already served in the real corpus** and contribute no frontier row
at all, so a decidable count is not a gainable-row count. The only rows that
are work are the ones the pin carries as `no recipe in corpus`, cross-tabbed
against the census verdict for the same export. `scratchpad/xtab.py` does
exactly that and nothing else.

The two populations turn out to barely overlap, which is why reading a census
summary as a work estimate — which the ranking before this did — inverts the
answer.

## Measured

| cluster | pinned rows | decidable | census refused |
| --- | ---: | ---: | ---: |
| `@tanstack/store@0.11.1` | 80 | 0 | 80 |
| `@solid-primitives/utils@6.4.1` | 126 | 0 | 126 |
| `@corvu/utils@0.4.2` | 98 | 6 | 92 |
| `@floating-ui/utils@0.2.12` | 96 | 0 | 96 |
| | **400** | **6** | **394** |

**Six of four hundred rows are recipe work.** The rest are census refusals
wearing the `no recipe in corpus` label.

### `@tanstack/store` — 80 rows

`flush` (40) and `toObserver` (40), both refused on
`property-access-unknown-accessor (PropertyAccessExpression)`. Eight further
exports are decidable and carry no frontier row.

### `@solid-primitives/utils@6.4.1` — 126 rows

`defaultEquals` (42) and `tryOnCleanup` (42) refused on
`domain-exhaustiveness "implementationUnavailable"`; `defer` (42) on
`property-access-unknown-accessor (ElementAccessExpression)`. Forty-one further
exports — `access`, `chain`, `clamp`, `pipe`, `noop` and the rest — measure
decidable and carry **no frontier row**. The census reports 394 decidable
candidates on this case; their contribution to the 1,470 is zero.

### `@corvu/utils@0.4.2` — 98 rows

`combineStyle` (66) refused on `property-access-unknown-accessor
(SpreadAssignment)` — the reason behind a refusal this phase had recorded
twice without naming. `getScrollAtLocation` (16) on `iteration-protocol
(ArrayBindingPattern)`; `default` (10) mixed with `jsx-element (JsxFragment)`.
Only `createKeyedContext`, `getKeyedContext` and `useKeyedContext` — 6 rows —
are decidable. Seventy-nine census candidates carry no frontier row.

### `@floating-ui/utils@0.2.12` — 96 rows

Entirely refused, across four shapes: `property-access-unknown-accessor` on
`PropertyAccessExpression` (40) and `BindingElement` (8), `instanceof
(BinaryExpression)` (32), and `coercion (BinaryExpression)` (16). Seventy-four
census candidates carry no frontier row.

## Why the frontier is mostly refusals

A decidable candidate gets closed, by a hand recipe or by synthesis, and stops
being a frontier row. What accumulates is what the census *refuses* — and a
refusal is invisible in the pin, because planning withholds a candidate for
want of a recipe **before** the census is consulted for it. Scaffolding is what
forces the census to run and unmasks it.

So `no recipe in corpus` in the pinned report is not evidence that a recipe is
the missing piece. On the two clusters measured it never was.

## What this corrects

**Tier C.** `2026-09-14-tier-c-class-census.md` proposed a hand recipe per
class export, "four exports, one artifact case for eighty of the rows". Its
`callSignatureNotUnique` refusals are indeed gone — ADR 0105 lifted them, as
that document predicted, and `Store`, `ReadonlyStore`, `batch`, `createAtom`,
`createAsyncAtom` and `createStore` now measure decidable. But the eighty rows
were never theirs: they are `flush` and `toObserver`, both refused. Those four
recipes would close **zero** rows.

**`defer`.** Ranked at 62 rows as "B-5 plus a two-arm join". It refuses on
`property-access-unknown-accessor (ElementAccessExpression)` — the same premise
as `flush` and `toObserver`, not a separate small item. The ×62 multiplicity
shared by `defaultEquals`, `defer` and `tryOnCleanup` was a real signal, but it
is two premises split differently than the export names suggest.

## The premise ranking, measured

| rows | form | shape |
| ---: | --- | --- |
| 120 | `property-access-unknown-accessor` | `PropertyAccessExpression` |
| 84 | `domain-exhaustiveness` | `"implementationUnavailable"` |
| 66 | `property-access-unknown-accessor` | `SpreadAssignment` |
| 42 | `property-access-unknown-accessor` | `ElementAccessExpression` |
| 32 | `instanceof` | `BinaryExpression` |
| 16 | `iteration-protocol` | `ArrayBindingPattern` |
| 16 | `coercion` | `BinaryExpression` |
| 10 | `jsx-element` | `JsxFragment` |
| 8 | `property-access-unknown-accessor` | `BindingElement` |
| 6 | *(decidable — recipe work)* | |

Grouped by form family, `property-access-unknown-accessor` is **236 of the 400
rows measured, 59%**. It is one refusal — "states no reviewed subject root, so
whose value it reads is undecided" — over four different AST shapes, and the
subject-root machinery is per-shape: ADR 0104 added one arm for a dependency
member, ADR 0106 another for a rest-parameter alias. So this is one family and
probably four premises, not one, and the per-shape row counts above are what
each is worth.

**Nothing here should be built from yet.** `implementationUnavailable` means
the selected signature's implementation declaration has no available body; why
that fires on ordinary `dist/*.js` exports is not established, and asserting a
cause from reading source rather than from a focused fixture is the error this
phase has already made twice. The fixture comes first, for each shape.

## Coverage

400 of 1,470 rows (27%) re-censused, four clusters, four for four on "the
frontier rows are refusals". `@solid-primitives/utils@7.0.0-next.4` (142) and
the `@corvu-next` pair (140) are in progress and would take it past 46%.
`motion-utils` (234) is settled independently by a focused producer fixture and
needs no re-census; with it, 634 of the 1,470 have a measured cause.
