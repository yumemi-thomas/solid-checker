# ADR 0115: A `returns` closure over the caller's arguments and fresh arrays of them

- Status: accepted and implemented (2026-09-23); written with the implementation
- Date: 2026-09-23
- Owners: the Type Facts producer's control-flow census
  (`invocation_transcripts.go`, handshake protocol 61), the Oxc facts
  (`ConditionalExpressionFact`, facts schema 43), the generator's
  argument-container walk (`returns_walk.rs`, `demand_plan.rs`,
  `inferred_contract.rs`), the semantic model's `argument-array` value shape,
  the policy-2 `returns` census, positive fact and synthesized veto
  (`type_facts.rs`, `synthesized_vetoes.rs`), and the consumer's `returns`
  projection (`contracts.rs`)
- Relation: the fifth decidable shape of ADR 0035's `returns` census, beside
  its empty closure, ADR 0075's whole-parameter identity, ADR 0109's merged
  props root and ADR 0113's plain return. It widens ADR 0075 from one parameter
  to a union, and its veto follows ADR 0096's identity pattern.

## Context

After ADR 0113, five exports had `returns` as their only open consumer domain
(`phase22/2026-09-23-what-holds-an-import-open.md` § What is left for a
`returns` shape), and `@solid-primitives/utils`' `asArray` was the largest at 51
sites:

```js
const asArray = (value) => Array.isArray(value) ? value : value ? [value] : [];
```

The generator proposed ADR 0113's plain return for it, and the census refused
it, correctly: the completion is an object. What it returns is exactly
describable -- the caller's argument, a fresh empty array, or a fresh array
holding the argument -- but no shape could say so, and nothing on the producer's
side enumerated a returned conditional's branches: `ReturnSite::sources` is
explicitly not exhaustive.

## Decision

**`returns` closes over one `return` operation per argument container, each
with an exact output: `parameter i` (the caller's argument at `i`, itself) or
`argument-array [i, …]` (a fresh array whose elements, in order, are the
caller's arguments at those indices). The census decides the enumeration from
the producer's arms of every return.**

### Why one operation per container, and a new shape

A union written as one `return` whose output is a `choice` of alternatives, or a
fresh array written as a `tuple` of parameters, would be the same claim spelled
with knowledge sets. Each set is a closure of its own (`choice-alternatives`,
`tuple-items`) that the proposal boundary weakens into a candidate, and no
census decides a value closure on an operation's output: the certifier refuses
them by design, since it has no enumeration to compare against. With those
candidates withheld, a consumer would be left an open union, which says nothing.
So the enumeration lives where a census already decides one, in the `returns`
domain's list of operations, and the array is a new shape,
`ValueShape::ArgumentArray { items }`, that is exact by construction and
carries no knowledge set. It is appended to the canonical encoding (tag 18) and
to the wire (`{"kind": "argument-array", "items": […]}`, `items` required); no
earlier document can carry it, so it needs no digest family.

### The producer states each return's arms (protocol 61)

`ReturnSite::arms` lists the values a returned expression can evaluate to when
it is a conditional or an array literal: the leaves of the conditional tree,
after identity-preserving wrappers, with a branch a literal condition excludes
left out -- the same reading `carriedCallableLocationsLocked` gives the tree.
Each arm carries its own value fact and the producer's unwritten-parameter
identity, and an array-literal arm lists its elements the same way, a spread
flagged. **Present means exhaustive**: a conditional evaluates to one of its
branches, and a tree past the producer's bounds (sixteen arms, depth eight)
states no arms at all rather than some. An `async` function states none: it
hands its caller a promise.

### What the census decides

The fifth arm of `census_returns_domain`, for a claim whose every item is a
`return` with an argument-container output, no two alike, and which is not a
lone whole parameter (ADR 0075's). Premises, each refusing by name:

1. a call, a plain completion form, and a present, classified control-flow
   census (`require_plain_classified_completion`);
2. every value-carrying return the producer did not prove unreachable is
   accounted for value by value -- by each of its arms when it has some, by its
   own whole-parameter identity otherwise -- and each value is the caller's
   unchanged whole argument or an array literal every element of which is one;
3. each such value is a container the claim enumerates;
4. each container the claim enumerates is handed back by at least one such
   value, so a positive claim about any one of them is witnessed.

A valueless completion is no `return` operation (`semantic-model.md` § returns)
and needs no disposition. Each operation's positive fact reads the same
evidence (`argument_container_return_sites`) rather than ADR 0075's "every
completion hands back this parameter", which a union does not claim.

### The veto

`Observation::ArgumentContainers` is synthesized for the closure (ADR 0036):
identity samples for every claimed index, and the marker
`return-outside-containers` when a normal completion is neither the argument at
a claimed index by SameValue nor an object whose `length` and elements are the
claimed arguments in order by SameValue. Both are written with operators and a
plain loop, so an intrinsic replaced during the subject's import cannot bend
them. The array half is not exact -- an array-like object passes -- and says so;
the census proves the literal.

### The generator proposes; the Oxc facts feed it

`returns_walk::argument_container_return` walks a function's own completions:
a returned conditional's branches (`ConditionalExpressionFact` now records the
elements of a branch that is an array literal, facts schema 42 -> 43), a
returned array literal's elements, and each identifier resolved by symbol to
one of the function's own whole parameters. The demand plan asks for those
symbols, which it did not before: only returned identifiers and the elements of
a returned literal were demanded. At least two distinct containers are needed;
anything else is "do not propose". The proposal goes before ADR 0113's plain
return, as the narrower claim.

### The consumer reads it

The consumer's return is a single reactive leaf. A closed claim whose every
return's output is exact -- `plain`, a whole `parameter` or an `argument-array`
-- and whose projections do not all agree on one leaf reads as describing no
reactive return, exactly as the consumer reads a local conditional whose
branches disagree. A return whose output projects to nothing (`[]`) is part of
the union too: reading the claim as its one surviving leaf would say the export
always returns that. A lone `argument-array` is a tuple of argument leaves.
Contract returns are only ever read to *find* a reactive leaf (a store, an
accessor, a merged props root), so this reading can hide a finding and never
invent one; that is the approximation, and it is the consumer's own.

## What still refuses

- **A call of the argument**: `access`'s and `accessWith`'s other branch
  (`v()`, `valueOrFn(...args)`) is the result of invoking the caller's value,
  and no value shape says so yet. Both keep their plain proposal, refused.
  `access` would stay at some uses anyway: its `callbacks` closure is refused.
- A parenthesized returned conditional, a logical `||`/`&&`/`??`, and a
  returned `[]` on its own: the walk does not read them, so nothing is proposed.
- A written parameter, an element that is not an argument, a spread or a hole,
  and a claim naming a container no live completion hands back.

## Consequences

- Contract corpus: `implementation-census-creates`' `typedCoercion`, a clamp
  over its three parameters, proposes those three returns instead of a plain one
  and certifies them; the new `implementation-census-argument-returns` pins the
  walk. Nothing else moves. Possible operations 455 -> 470, proof candidates
  1,553 -> 1,581.
- The tracer certifies `asArray`, `pick` and `pairOrValue` against the real
  producer and refuses four wrong claims by name
  (`the_argument_container_census_certifies_exactly_the_enumerated_containers`);
  `fixtures/reactive-ir/package-argument-container-consumer` is the consumer's
  half (coverage 84 projects, 443 findings; the stable-main ledger moves to 142).
- Handshake protocol 60 -> 61 and facts schema 42 -> 43: a rebuilt
  `bin/solid-typefacts` and new certification pins. No existing document's
  digest moves.
