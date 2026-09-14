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

## Validity, checked before the numbers

Two failure modes of this harness answer instead of erroring, and both were hit
here, so every run below is validated before it is counted:

- **A run whose cases are not the pin's cases.** `@solid-primitives/utils@7.0.0-next.4`
  certified through `motion-solidjs@0.7.0-beta.4` with `status: certified` and
  **zero** withheld closures — which reads as "nothing to do" and is in fact a
  case set that never contained the closures. Zero overlap with the five
  artifact cases the pin carries for it. **Discarded, not reported.**
- **A verdict attributed across cases.** A pinned row is matched to a verdict
  by `(artifactCase, export)` first; only where that case was not reproduced is
  the export's verdict from another case used, and the count of each is given.

| cluster | pin cases | reproduced | rows | case-exact | export-inferred |
| --- | ---: | ---: | ---: | ---: | ---: |
| `@tanstack/store@0.11.1` | 1 | 1 | 80 | 80 | 0 |
| `@solid-primitives/utils@6.4.1` | 1 | 1 | 126 | 126 | 0 |
| `@corvu/utils@0.4.2` | 13 | 6 | 98 | 87 | 11 |
| `@floating-ui/utils@0.2.12` | 2 | 2 | 96 | 96 | 0 |
| `@corvu-next/utils@0.1.4` | 14 | 6 | 88 | 46 | 42 |
| | | | **488** | **435** | **53** |

## Measured

| cluster | rows | decidable | refused | mixed |
| --- | ---: | ---: | ---: | ---: |
| `@tanstack/store@0.11.1` | 80 | 0 | 80 | 0 |
| `@solid-primitives/utils@6.4.1` | 126 | 0 | 126 | 0 |
| `@corvu/utils@0.4.2` | 98 | 6 | 87 | 5 |
| `@floating-ui/utils@0.2.12` | 96 | 0 | 96 | 0 |
| `@corvu-next/utils@0.1.4` | 88 | 56 | 14 | 18 |
| | **488** | **62** | **403** | **23** |

**62 of 488 rows — 13% — are recipe work.** The first four clusters measured
were 6 of 400, and reporting that as the shape of the frontier was premature:
`@corvu-next/utils` is 64% decidable on its own and moved the figure by a
factor of ten. Four clusters agreeing is not the population.

The direction still holds — 83% of measured rows are census refusals wearing
the `no recipe in corpus` label — but the recipe share is an order of magnitude
larger than four clusters suggested, and no further cluster should be
extrapolated from.

### Where the decidable rows are

All of them are in the `@corvu-next` pair: `dataIf`, `isButton`, `isFunction`
(10 each), `createKeyedContext`, `getKeyedContext`, `useKeyedContext` (8 each
there, 2 each on `@corvu/utils`), and `default` (18, mixed with a
`coercion (PostfixUnaryExpression)` refusal). These are ordinary hand recipes
and the only measured recipe work in the frontier so far.

### Where the refusals are

`@tanstack/store`: `flush` and `toObserver`, 40 each, on
`property-access-unknown-accessor (PropertyAccessExpression)`.

`@solid-primitives/utils@6.4.1`: `defaultEquals` and `tryOnCleanup`, 42 each,
on `domain-exhaustiveness "implementationUnavailable"`; `defer`, 42, on
`property-access-unknown-accessor (ElementAccessExpression)`.

`@corvu/utils` and `@corvu-next/utils`: `combineStyle`, 80 across both, on
`property-access-unknown-accessor (SpreadAssignment)` — the reason behind a
refusal this phase had recorded twice without naming. `getScrollAtLocation`,
18, on `iteration-protocol (ArrayBindingPattern)`.

`@floating-ui/utils`: entirely refused across four shapes —
`property-access-unknown-accessor` on `PropertyAccessExpression` (40) and
`BindingElement` (8), `instanceof (BinaryExpression)` (32), and
`coercion (BinaryExpression)` (16).

On every case the census also reports decidable candidates that carry **no
frontier row at all** — 79 exports on `@corvu/utils`, 74 on `@floating-ui/utils`,
41 on `@solid-primitives/utils` — because a decidable candidate gets closed and
stops being a frontier row. That gap is what makes a raw decidable count
useless as a work estimate.

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
| 80 | `property-access-unknown-accessor` | `SpreadAssignment` |
| **62** | *(decidable — hand recipes)* | |
| 42 | `property-access-unknown-accessor` | `ElementAccessExpression` |
| 32 | `instanceof` | `BinaryExpression` |
| 18 | `iteration-protocol` | `ArrayBindingPattern` |
| 18 | `coercion` | `PostfixUnaryExpression` *(mixed with decidable)* |
| 16 | `coercion` | `BinaryExpression` |
| 8 | `property-access-unknown-accessor` | `BindingElement` |
| 8 | `jsx-element` | `JsxFragment` *(5 mixed)* |

Grouped by form family, `property-access-unknown-accessor` is **250 of 488,
51%**. It is one refusal — "states no reviewed subject root, so whose value it
reads is undecided" — over four AST shapes, and the subject-root machinery is
per-shape: ADR 0104 added an arm for a dependency member, ADR 0106 another for
a rest-parameter alias. So this is one family and probably four premises, and
the per-shape counts are what each is worth.

**Nothing here should be built from yet.** `implementationUnavailable` means
the selected signature's implementation declaration has no available body; why
that fires on ordinary `dist/*.js` exports is not established, and asserting a
cause from reading source rather than from a focused fixture is the error this
phase has already made twice. The fixture comes first, for each shape.

## Coverage

488 of 1,470 rows (33%) re-censused across five clusters.
`@solid-primitives/utils@7.0.0-next.4` (142) is outstanding — the graph-lane
attempt through `motion-solidjs` was invalid, and 34 of its rows are a
`reused-proposal` root row that can be certified directly. `motion-utils` (234)
is settled independently by a focused producer fixture and needs no re-census;
with it, 722 of the 1,470 have a measured cause.
