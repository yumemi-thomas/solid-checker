# ADR 0090: A data-only literal parameter default

- Status: accepted and implemented (2026-09-11); written with the
  implementation
- Date: 2026-09-11
- Owners: Type Facts producer (`uncensused_invoking_forms.go`), certifier
  (`type_facts.rs`)
- Relation: joins ADR 0034's premise with ADR 0044's over the two arms of one
  parameter. Handshake protocol 46 → 47.

## Context

ADR 0034 excludes a parameter that carries a default, and gives the reason in
`defaultedFromModuleValue`'s own fixture comment:

> A parameter whose default is a value *this* module made. When the caller
> omits the argument the accessor that may run is this module's own, which is
> exactly what ADR 0034 excluded a defaulted parameter for.

That is right about a default naming a module value. It is not right about
every default, and the exclusion is uniform. ADR 0043 relaxed it once, for a
default *naming another rooted parameter* — `parameter-default`, caller-rooted
on both arms.

The shape it still refuses is one of the most common in JavaScript:

```js
export function makeRetrying(fetcher, options = {}) {
  const delay = options.delay;
```

`options.delay` is an uncensused `property-access-unknown-accessor`, the
parameter roots nowhere, and `creates` withholds.

## Decision

A parameter roots at **its own slot** under the new derivation
`parameter-default-literal` when

- it carries a default that is a **data-only literal** — the predicate ADR 0044
  already uses, so no `get`/`set` member and no `__proto__:` member;
- its binding is a plain identifier, not a rest parameter, and
- the body **writes** it nowhere.

Such a parameter holds exactly one of two values, and each is excused by a
premise this build has already reviewed:

- the caller's argument, which is ADR 0034's premise verbatim;
- the object the default expression freshly created, which is ADR 0044's.

The arms are **exhaustive**: a defaulted parameter is the argument when it is
not `undefined`, and the default otherwise. No flow analysis is involved and
none is claimed — whichever arm supplied the value, an accessor read on it is
excused, so which one it was never comes up.

## Why a new spelling rather than `parameter-default`

`parameter-default` is caller-rooted on *both* arms. This one is not: one arm is
this program's own object. A consumer reading this derivation as that one would
record "the caller installed whatever ran", which is false on the default arm.
ADR 0043's own rule applies — a consumer must refuse a spelling it has not
reviewed rather than read it as a neighbour — so the handshake moves and the
certifier gains its own disposition pair,
`parameter-default-literal-accessor` and its `-write` companion.

## What is deliberately excluded

- **Propagation through a local binding.** `rootLocalDeclarationsLocked`
  refuses this root explicitly. What a *property* of the default's literal
  holds is an arbitrary expression of this program's — `const d = options.delay`
  under `options = { delay: makeThing() }` names one — which is the same reason
  ADR 0044 roots a direct reference alone.
- **Form kinds other than the accessor pair.** An iteration protocol or an
  `instanceof` over such a parameter refuses. The second arm has only been
  argued about here for property and element reads.
- **A written parameter.** An assigned value is neither the caller's argument
  nor the default's literal, so there is no second arm left to join.
- **A literal carrying an accessor.** Then the default arm runs code this
  program installed, and the premise that made it safe is gone.

## Consequences

Measured on the 2026-09-11 corpus: `property-access-unknown-accessor` distinct
claims 122 → 118, census-refused distinct claims 397 → 395, certified closures
5,187 → 5,190. Three rows moved, all gaining; no row lost a closure and no
status changed.

That is a small gain for a protocol bump, and it is recorded as such. The
premise was chosen because it is the one shape a reading of the corpus could
confirm — see
`docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`
§ 58 — not because it was predicted to be the largest. The 118 that remain are
still undiagnosed, and § 57.4's prerequisite stands: the producer knows which
leg each fell off and does not say it across the wire.
