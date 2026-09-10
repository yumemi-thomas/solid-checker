# `reads` census: the admission review, and why it fails today

- **Status:** review result. No production code, dialect table, audit,
  contract or fixture was changed by this pass.
- **Date:** 2026-09-10.
- **Question asked:** implement the `reads` census (roadmap lever F,
  [implementation-census-plan](2026-09-03-implementation-census-plan.md) § 4.4),
  the domain that gates every finding after the
  [findings-delta measurement](../2026-09-10-findings-delta-measurement.md):
  `reads` is closed for 0 of 8950 corpus exports.
- **Answer:** the census cannot be admitted yet, and the blocker is the first
  prerequisite rather than the census machinery. The audited documents close
  `reads` on four canonical `solid-js` primitives whose own summaries guard on
  a props-proxy access — the non-call read form `semantic-model.md` § reads
  calls load-bearing — and *no* bundled document models a props access as a
  read at all. The audits and the model disagree about what `reads` covers.
  Until that is decided, the domain's closures deny nothing the dialect table
  can restate, exactly as with `returns`.

## 1. What the census needs, in order

`Solid2::NEGATIVE_ROWS`
([solid_2.rs:271](../../../rust/crates/solid-dialect/src/solid_2.rs)) states
the sequence itself:

> **`reads`, `writes`, `invalidates`, `cleanups`, `disposals`.** No
> counter-example found, and no positive review performed either. They are
> silent because nothing here has read them […] a domain is added when it is
> audited, not when it is convenient.

So three gates, in order:

1. **Dialect negative rows for `reads`** — the audited documents' `reads`
   closures must be admissible as negative authority. Most rows are mechanical:
   the citation test re-derives the closure from the cited bytes. Admission is
   a review for counter-examples. **This document performs that review.**
2. **A reviewed veto observation for `reads`** —
   `synthesized_vetoes::reviewed_observation` answers only `returns` and
   `creates`. ADR 0036 keeps an unreviewed domain's candidates withheld for
   want of a recipe rather than gating them on the wrong observation, so
   without one the census closes nothing even with rows.
3. **The census predicate itself**, which § 4.4 describes as "same machinery,
   plus the proxy property-access forms".

Gate 3 is engineering. Gates 1 and 2 are review decisions.

## 2. Gate 1 fails: four canonical primitives close `reads` while guarding on props

The model's § reads
([semantic-model.md](../semantic-model.md)) defines the domain to include reads

> arising from non-call syntax: **a property access on a store, props, or
> projection proxy**, a whole-object observation (`$TRACK`, spread,
> `Object.keys`, `in`, iterating a store), and a JSX attribute or child
> expression whose lowering reads an accessor.

and settles the point explicitly: **[Decision 2026-09-03]** *"the proxy
property access is the load-bearing non-call form, and it is the reason a
`reads` census can never be a census of calls alone."* A read stays a read
whatever its tracking relation — "an `untracked` or `ambient-at-execution` read
is still a read and only the dependency it registers differs".

Scanned mechanically over every bundled document: summaries that close `reads`
with an empty collection *and* whose own `cases`/`operations` guards branch on
an argument path.

| document | shape | guard atom | exports | counter-example? |
| --- | --- | --- | --- | --- |
| `solid-js` | **component** | `arg0.keyed` | **`For`, `Match`, `Repeat`, `Show`** | **yes** — arg 0 of a component is props, a proxy |
| `solidjs-web` | component | `arg1.lazy` | `clientOnly` | probable; arg 1 of a `component` shape needs the audit to say what it binds |
| `solid-js` | plain | `arg2.defer` | `createEffect` | no — a plain options object is not a proxy |

For `Show`, the tension is inside one summary object: `closed` lists `reads`,
`reads` is `[]`, and both published operations carry
`guard.all = [{arg: 0, path: ["keyed"], literal: …}]`. The runtime cannot
select between `accessor-child` and `raw-child` without reading
`props.keyed`. `For`, `Match` and `Repeat` share the summary.

### 2.1 It is a convention conflict, not a plain audit error

A corroborating scan changes the character of this finding and is the reason
the remedy is a decision rather than a correction: **no bundled document
anywhere models a props access as a `reads` item.** Every `read` operation in
every document reads a *reactive resource* —

| document | operation | inputs |
| --- | --- | --- |
| `solid-js` | `latest-read`, `pending-read` | `reactive` accessor of `async-target` |
| `solidjs-signals` | `read-store`, `deep-read`, `shallow-read`, `snapshot-read` | resource reads |
| `solidjs-web--web-node-server` | `claim-stream` | — |

So the audits are internally consistent with each other and inconsistent with
`semantic-model.md` § reads. Two readings, and the difference is a model
question:

- **The model as written.** § reads names "a property access on a store,
  **props**, or projection proxy" among the non-call forms and settles
  **[Decision 2026-09-03]** that the proxy property access is *"the
  load-bearing non-call form […] the reason a `reads` census can never be a
  census of calls alone"*. Under this reading `Show`'s closure is false and the
  four rows are counter-examples.
- **The audits as practised.** A component's props getters are installed by the
  *caller's* compiled JSX, so reading `props.keyed` runs the caller's
  expression — arguably the same exclusion § 3.2 of the census plan applies to
  a callee rooted at a parameter. Under this reading the closure is correct and
  the model's sentence naming props is what needs narrowing.

Nothing in the repository resolves this. That is itself disqualifying for
gate 1: negative authority cannot be granted for a domain whose closure
convention is unsettled — the same reason `returns` is withheld, where the doc
records that the audits "are using `returns` for emission-like operations […]
Until that convention is reconciled with the model, the domain's closures deny
nothing this table can restate."

**The remedy is a model decision first, then a re-audit.** Deciding for the
model as written also decides most of § 4.4's "plus the proxy property-access
forms": if props access is a read, the census must disposition every property
access on a possibly-proxy value, which is the work lever C's type premises
make decidable at all. Deciding for the audits shrinks the census to something
much closer to the `creates` machinery — and shrinks what a closed `reads`
claim is worth to a consumer, which is the trade to weigh.

## 3. Gate 2: a veto observation for `reads` is designable, and partial

Not blocked the way `disposals` and `invalidates` are. Those need private
runtime internals to observe at all. `reads` has a route that uses public API
only, because the synthesized veto already samples arguments from the
export's declared signature: **hand the export instrumented accessors as its
arguments and observe whether it calls them.**

That falsifies the parameter-read half of the domain exactly — which is the
half the consumer projection actually reads back
(`project_reactive_reads` keeps `ValueShape::Parameter { index, path }` and
`ValueShape::Reactive`). It cannot observe a read of a source the export owns
internally, so like `creates`' globalThis observation it is a partial
falsifier and must say so in its `observation` string. That is sound in this
architecture — the census proves the closure, the veto only attempts
contradiction — but it is a review decision, not an implementation detail.

Reviewing that design is the cheapest next step if the re-audit lands.

## 4. What was not done, and why

No rows were added, no observation was registered, and no census code was
written. Writing gate 3 against gates 1 and 2 would produce a code path that
provably decides nothing, and adding rows without the re-audit would be
precisely the "blanket trust to make a fixture green" AGENTS.md forbids.

## 5. Reproducing the scan

~~~sh
for f in pkg/contracts/bundled/solid-v2/*.json pkg/contracts/bundled/solid-v1/*.json; do
  b=$(basename "$f" .json); [ "$b" = bundle-index ] && continue
  jq -r --arg f "$b" '
    . as $d
    | [ ($d.entrypoints[].cases[]?.exports // {}) | to_entries[] ] as $ex
    | ($d.summaries // {}) | to_entries[] | . as $s | ($s.value.call // {}) as $c
    | select((($c.closed // []) | index("reads")) != null and (($c.reads // []) | length) == 0)
    | [ ($c.cases // [])[] | (.when.all // [])[] | select(.arg != null and (.path|length)>0) | "arg\(.arg).\(.path|join("."))" ]
      + [ ($c.operations // [])[] | (.guard.all // [])[] | select(.arg != null and (.path|length)>0) | "arg\(.arg).\(.path|join("."))" ]
    | unique as $atoms | select(($atoms|length)>0)
    | "\($f)\t\($s.value.shape)\t\($atoms|join(","))\t\([ $ex[] | select(.value == $s.key) | .key ] | unique | join(", "))"
  ' "$f"
done | sort -u
~~~

## 6. Addendum (2026-09-10): the decision as put collides with ADR 0034/0040

Asked to resolve § 2.1, the owner chose **the model as written** — a props
access is a read. Taken literally that is not implementable, for three reasons
found while scoping the census against it.

**1. It contradicts two accepted, implemented ADRs.**
[ADR 0034](../../adr/0034-parameter-rooted-accessor-disposition.md) settled the
attribution question for a parameter-rooted accessor:

> Code the caller attached to an object it passed is not this export's
> registration any more than a callback it passed is; if such a getter calls
> `createEffect`, that call is in the caller's file, where ordinary analysis
> already sees it with its own owner and timing.

[ADR 0040](../../adr/0040-parameter-rooted-accessors-in-write-position.md)
extended it to write position. A component's props getters are installed by the
*caller's* compiled JSX, so `props.keyed` inside `Show` is caller code by
exactly that argument. Under these ADRs `Show`'s `reads: []` is **correct**.

**2. The producer cannot supply the fact.** Stated identically in
`invocation.go:629` and `invocation.rs:815`, deliberately:

> A `Proxy` trap … is a property of the object a value happens to be at
> runtime, not of any syntax, so no walk of an implementation can see it:
> `obj.x` on a proxy is the same `PropertyAccessExpression` as `obj.x` on a
> plain object. It is out of the producer's reach entirely… A consumer whose
> claim requires that no proxy trap ran must obtain that premise elsewhere.

**3. Types cannot substitute.** Solid types a store as its plain object type
(`Store<T>` is `T`) and props as a plain props object, so lever C's declared
signature premise — the thing that made `creates` decidable — cannot separate a
store from a plain object for exactly the values that matter. A census obliged
to prove "no proxy access occurred" would refuse every export that touches a
parameter's properties, permanently, by definition rather than by engineering
gap.

### The reconciliation, and it is what the audits already do

Scope the rule by *whose value the receiver is* — the same axis ADR 0042 and
ADR 0044 already run the census on:

- A property access on a **parameter-rooted** receiver (props, a store the
  caller passed) is the **caller's** read. Not this export's `reads`. Consistent
  with ADR 0034/0040 and with the callable carve-out § reads already has.
- A property access on a proxy **this export created or imported** — its own
  store, its own projection — is the export's `reads`. This is the model's
  load-bearing form, and [ADR 0044](../../adr/0044-a-value-this-program-built.md)
  already supplies the "a value this program built" root derivation that
  decides it.

Every modelled `read` operation in every bundled document is already of the
second kind — a reactive-resource read, never a parameter path — so the audits
need no correction under this rule, and `semantic-model.md` § reads needs one
sentence: the parameter-rooted carve-out, spelled the way the callable one is.

**What each choice costs.** The reconciled rule makes the census the `creates`
machinery plus own-resource proxy accesses, buildable on ADR 0044's existing
derivation. The literal rule requires a producer fact the producer says it
cannot provide, and would make `reads` unclosable for most exports — which is
where the domain already is, at 0 of 8950.
