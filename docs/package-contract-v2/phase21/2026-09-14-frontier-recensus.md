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
| `@tanstack/store@0.11.1` | 80 | **0** | **80** |
| `@solid-primitives/utils@6.4.1` | 126 | **0** | **126** |
| | **206** | **0** | **206** |

Not one of the 206 is recipe work.

### `@tanstack/store` — 80 rows

| rows | export | verdict |
| ---: | --- | --- |
| 40 | `flush` | refused: `property-access-unknown-accessor (PropertyAccessExpression)` |
| 40 | `toObserver` | refused: `property-access-unknown-accessor (PropertyAccessExpression)` |

Eight further exports are decidable and carry no frontier row.

### `@solid-primitives/utils@6.4.1` — 126 rows

| rows | export | verdict |
| ---: | --- | --- |
| 42 | `defaultEquals` | refused: `domain-exhaustiveness "implementationUnavailable"` |
| 42 | `tryOnCleanup` | refused: `domain-exhaustiveness "implementationUnavailable"` |
| 42 | `defer` | refused: `property-access-unknown-accessor (ElementAccessExpression)` |

Forty-one further exports — `access`, `chain`, `clamp`, `pipe`, `noop` and the
rest — measure decidable and carry **no frontier row**. The census reports 394
decidable candidates on this case; their contribution to the 1,470 is zero.

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

## The premise ranking so far

| premise | measured rows | where |
| --- | ---: | --- |
| `property-access-unknown-accessor` subject root | 122 | `defer` 42, `flush` 40, `toObserver` 40 |
| `domain-exhaustiveness: implementationUnavailable` | 84 | `defaultEquals` 42, `tryOnCleanup` 42 |

Both are premises, not recipes, and neither has a fixture yet. `shallow` (40
rows, already unmasked in the pin as refused) and `EventClient` (11,
`implementationUnavailable`) belong to the same two families, which would take
them to 162 and 95 — but those are read off the pin rather than re-censused,
so they are not counted above.

**Nothing here should be built from yet.** `implementationUnavailable` means
the selected signature's implementation declaration has no available body; why
that fires on ordinary `dist/*.js` exports is not established, and asserting a
cause from reading source rather than from a focused fixture is the error this
phase has already made twice. The fixture comes first.

## Coverage

206 of 1,470 rows (14%) re-censused. `@corvu/utils` (98) and
`@floating-ui/utils` (96) are in progress; `@solid-primitives/utils@7.0.0-next.4`
(142) and the `@corvu-next` pair (114) are the next largest. `motion-utils`
(234) is settled independently by a focused producer fixture and needs no
re-census.
