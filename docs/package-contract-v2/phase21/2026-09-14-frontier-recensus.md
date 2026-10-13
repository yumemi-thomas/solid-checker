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
  artifact cases the pin carries for it. Discarded, and re-run on that
  package's own `reused-proposal` root row, which reproduces two of the five.
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
| `@solid-primitives/utils@7.0.0-next.4` | 5 | 2 | 142 | 34 | 108 |
| | | | **630** | **469** | **161** |

`@solid-primitives/utils@7.0.0-next.4` is the weakest attribution here — 108 of
its 142 rows are export-inferred. It is corroborated rather than trusted: the
three exports it shares with the case-exact `6.4.1` run — `defaultEquals`,
`tryOnCleanup`, `defer` — draw the same premise in both, which is the only
reason its inferred rows are counted at all.

## Measured

| cluster | rows | decidable | refused | mixed |
| --- | ---: | ---: | ---: | ---: |
| `@tanstack/store@0.11.1` | 80 | 0 | 80 | 0 |
| `@solid-primitives/utils@6.4.1` | 126 | 0 | 126 | 0 |
| `@corvu/utils@0.4.2` | 98 | 6 | 87 | 5 |
| `@floating-ui/utils@0.2.12` | 96 | 0 | 96 | 0 |
| `@corvu-next/utils@0.1.4` | 88 | 56 | 14 | 18 |
| `@solid-primitives/utils@7.0.0-next.4` | 142 | 0 | 142 | 0 |
| | **630** | **62** | **545** | **23** |

**62 of 630 rows — 10% — are recipe work.** The first four clusters measured
were 6 of 400, and reporting that as the shape of the frontier was premature:
`@corvu-next/utils` is 64% decidable on its own and moved the figure by a
factor of ten. Four clusters agreeing is not the population.

The direction still holds — 87% of measured rows are census refusals wearing
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
| 124 | `domain-exhaustiveness` | `"implementationUnavailable"` |
| 122 | `property-access-unknown-accessor` | `PropertyAccessExpression` |
| 92 | `property-access-unknown-accessor` | `ElementAccessExpression` |
| 80 | `property-access-unknown-accessor` | `SpreadAssignment` |
| **62** | *(decidable — hand recipes)* | |
| 40 | `domain-exhaustiveness` | `"callSignatureNotUnique"` |
| 32 | `instanceof` | `BinaryExpression` |
| 26 | `coercion` | `BinaryExpression` |
| 18 | `iteration-protocol` | `ArrayBindingPattern` |
| 18 | `coercion` | `PostfixUnaryExpression` *(mixed with decidable)* |
| 8 | `property-access-unknown-accessor` | `BindingElement` |
| 8 | `jsx-element` | `JsxFragment` *(5 mixed)* |

Grouped by form family, `property-access-unknown-accessor` is **302 of 630,
48%**. It is one refusal — "states no reviewed subject root, so whose value it
reads is undecided" — over four AST shapes, and the subject-root machinery is
per-shape: ADR 0104 added an arm for a dependency member, ADR 0106 another for
a rest-parameter alias. So this is one family and probably four premises, and
the per-shape counts are what each is worth.

**Nothing here should be built from yet.** `implementationUnavailable` means
the selected signature's implementation declaration has no available body; why
that fires on ordinary `dist/*.js` exports is not established, and asserting a
cause from reading source rather than from a focused fixture is the error this
phase has already made twice. The fixture comes first, for each shape.

## The node kind is not the premise (fixtured 2026-09-14)

The ranking above groups by the AST node the form sits on, because that is what
the refusal names. That grouping is **not** the premise grouping, and the
largest row of it is the least homogeneous. Reading the actual source behind
every `property-access-unknown-accessor` refusal in the six clusters:

| export | what the subject really is |
| --- | --- |
| `toObserver` | a ternary over two parameters |
| `getNodeName` | `(param.x \|\| '')` — a parameter joined with an own literal |
| `flush` | an element of a module-level mutable array |
| `getComputedStyle` | a package-own function's call result |
| `getParentNode` | a local binding from a mixed `\|\|` chain |

`PropertyAccessExpression`'s 122 rows are at least five premises of ten to
forty rows each. `SpreadAssignment`'s 80 are the most homogeneous block in the
frontier, which makes it the right first target — but not for the reason the
ranking suggested.

### What the spread fixture found

`spread_assignment_subject_roots_test.go` probes the shape directly, and the
premise is not "a spread of a parameter": **the producer already states that
one.** What refuses is `written-parameter`.

~~~js
function combineStyle(a, b) {                     // @corvu/utils, 80 rows
  if (typeof b === "string") b = stringStyleToObject(b);
  return { ...a, ...b };                          // ...a roots; ...b refuses
}
~~~

| the parameter is assigned | roots? |
| --- | --- |
| nothing | `parameter` |
| itself (`b = b`) | `parameter` |
| another parameter (`b = a`) | **refuses** |
| an own literal (`b = { x: 1 }`) | **refuses** |
| a package-own function's result | **refuses** |
| `Object.create(null)` | **refuses** |
| a default (`b: any = {}`) | `parameter-default-literal` |

So one refusal spelling hides four premises worth very different amounts. The
narrowest — every assigned value is a parameter — is the argument ADR 0034
already makes, and it still refuses.

**It does not close `combineStyle`.** That second arm is
`stringStyleToObject(b)`, whose body returns a `const object = {}` it fills
itself: an own literal one function hop away. The eighty rows need the
parameter join **and** a hop through a package-own result — two premises
stacked, not the one the ranking implied. Anything costed off the node-kind
table above is costed too low.

## What the eighty rows are actually blocked on (fixtured 2026-09-14)

The section above concluded that `combineStyle`'s eighty rows need "the
parameter join **and** a hop through a package-own result — two premises
stacked". **That is wrong, and the error was a bad fixture.** The probe behind
it wrote the helper as `function toObject(s) { return { y: 2 }; }` — a literal
returned directly. `combineStyle`'s real helper returns a *binding*:

~~~js
function stringStyleToObject(style) {
  const object = {};
  while (match = re.exec(style)) { object[match[1]] = match[2]; }
  return object;                    // an unwritten, data-only literal binding
}
~~~

which is exactly what ADR 0093's local-literal-result premise requires. Probed
against the real shape, the producer states:

~~~
[0] root="parameter"                 (...a)
[1] root="parameter-or-own-result"   (...b)   localLiteralResults=1
~~~

**The producer already roots it.** ADR 0093 closed this shape in 2026-09-12, and
nothing in the Type Facts layer refuses these rows.

### The blocker is that the `reads` census has no call walk

`parameter-or-own-result` is consumed in exactly one place — `census_transcript`'s
*deferred* arm, which runs after the call walk has demanded the callees'
transcripts. It has to be deferred: confirming the premise means reading
`stringStyleToObject`'s own transcript. And `census_reads_domain` says in its
own comment why it cannot go there:

> No deferral. The `creates` census holds a coercion or a local literal result
> back until its call walk has demanded the callees' transcripts; **this census
> has no call walk**, so a form it cannot decide here it cannot decide at all.

So ADR 0093 is implemented for `creates` and unreachable for `reads`. The
eighty rows need the `reads` census to demand one hop of callee transcripts —
not a new premise, and nothing at all in the producer.

That capability is shared. The `call-result` refusals — `getComputedStyle`,
`isContainingBlock`, `isOverflowElement`, `sortBy`, 26 rows — are the same
missing hop, and so is any later premise whose confirmation lives in a callee.

### The two-parameter join, costed

The other candidate, measured at **40 rows** (`toObserver`'s
`(isObserver ? nextHandler.error : errorHandler)`), refuses as
`not-a-reference`: the subject is a ConditionalExpression, so the root walk
stops before it looks at the arms. Both arms do root at `parameter` — a
property chain on a parameter roots, and parentheses are transparent — but at
**different slots**, and the accessor family's wire carries a single
`subjectParameter`. Admitting it therefore needs a new plural field, a new
derivation, a handshake protocol bump, a matched producer/checker rebuild and a
corpus re-pin: the campaign's highest ceremony for its smallest gain, against
ADR 0103's 148 rows, 0104's 124, 0105's 80 and 0106's 62.

**Recommendation: the call walk, not the join.** It is checker-only, it closes
80 rows against 40, it needs no protocol bump, and it unlocks a class rather
than a shape.

## The tail is recipe work, at one recipe per row (2026-09-14)

Six tail clusters, all on the `reused-proposal` root-row lane, all validated
against the pin's artifact cases:

| cluster | rows | decidable | refused |
| --- | ---: | ---: | ---: |
| `@corvu-next/utils@0.1.5` | 24 | 21 | 3 |
| `@kobalte/utils@2.0.0-alpha.0` | 24 | 20 | 4 |
| `@tanstack/devtools-ui@0.7.1` | 23 | 21 | 2 |
| `@tanstack/devtools-utils@0.7.0` | 21 | 15 | 6 |
| `@solidjs/meta@0.29.4` | 16 | 11 | 5 |
| `@kobalte/solidbase@0.6.13` | 14 | 2 | 12 |
| | **122** | **90 (74%)** | 32 |

The head clusters measured 10% decidable. The tail measures **74%**, and that
is structural rather than noise: the head is a few closures of high
multiplicity each blocked by one premise, the tail is many distinct closures
each wanting its own veto.

### What it costs, measured

**93 decidable rows across 93 distinct (artifact case, claim) pairs — 1.0 rows
per recipe.**

Against ADR 0107's `combineStyle`, which was 90 rows in 8 recipes, 11.3 to 1.
The same instrument, the same domain, a fourteenfold difference in what a
recipe is worth. A hand-authored veto is not a fixed-value unit of work, and
any plan that prices recipes without this ratio is pricing the wrong thing.

Each of the 93 needs its own reading of the export's source, its own samples
measured against the installed package, and its own `NEVER EMITS` declaration
in the roster test. None of that is shared between them.

### The recipe floor, stated

This is the decision the opening plan deferred to a fifth step, now measurable
rather than estimated. The remaining frontier splits into:

- **Premise-shaped**, high multiplicity: `motion-utils` 234 rows in 9 closures,
  `property-access-unknown-accessor` ~212 across three shapes,
  `implementationUnavailable` 124. Expensive per premise, cheap per row.
- **Recipe-shaped**, unit multiplicity: the tail, at one recipe per row. Cheap
  per unit, and exactly as expensive per row as it is large.

Nothing makes the second class cheaper — no premise has leverage over a closure
that appears once. Writing them is a decision about how much hand-authoring the
frontier number is worth, not a technical question, and it should be taken
deliberately rather than drifted into one cluster at a time.

## Coverage

After ADR 0107 closed 90, the frontier is 1,380. 752 of those rows are
re-censused across twelve clusters (630 in the head six, 122 in the tail six);
596 remain un-censused, in a genuine tail whose largest cluster is 31 rows.
Originally, before ADR 0107: `motion-utils` (234)
is settled independently by a focused producer fixture and needs no re-census;
with it, **864 of the 1,470 — 59% — have a measured cause**. What remains
un-censused is the long tail: `@kobalte/utils` (55 rows in 55 closures),
`@kobalte/core` (29), `@solidjs/meta` (32), and forty-odd packages below 25
rows each, where the row-per-closure ratio approaches one and no premise has
leverage.
