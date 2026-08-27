# Package-contract semantic model

Status: **frozen for `semanticModelVersion: 1` on 2026-08-27**. Later wire
compression may change without changing this model. Any incompatible semantic
change requires a new semantic-model version and digest domain.

## Knowledge lattice

Each set-valued claim domain has items and immediate local completeness.

| Knowledge | Wire encoding | Normalized meaning |
| --- | --- | --- |
| Unknown | Domain absent and not named in local `closed` | No usable positive or negative fact |
| Partial positive | Non-empty domain present and not named in local `closed` | Listed items are known; other items may exist |
| Complete positive | Non-empty domain present and named in local `closed` | Listed items exhaust the domain |
| Complete negative | Empty domain present and named in local `closed` | The domain is proved absent |

An empty open collection is invalid because it carries no information and is
too easy for a generator bug to confuse with absence. A name in `closed`
requires its immediate sibling collection, even when empty.

Closure is never inherited through:

- parent or child objects;
- sibling domains;
- summary references;
- guarded alternatives;
- recursive value leaves;
- referenced resources.

## Positive strength

Operation cardinality distinguishes possible from guaranteed behavior:

```json
{
  "min": 0,
  "max": "many"
}
```

- `min: 0` means the behavior is possible and can prevent negative proof.
- `min >= 1` means the behavior is guaranteed under its guard.
- A finite maximum is a proved bound.
- `"many"` means repetition is unbounded.
- A missing bound is unknown.

Every bound has exactly one scope:

- `trigger`: per occurrence of the operation's trigger;
- `call`: across one invocation of the contracted export;
- `resource`: across the lifetime of one named resource.

Bounds with different scopes are not comparable and never merge implicitly.
An operation that runs once per async emission uses trigger scope; the stream's
total emission bound, when known, uses resource scope. A missing scope is
invalid whenever either bound is present.

A probe can witness possibility. Only replayable static proof can establish a
positive minimum, a finite maximum, or domain closure.

## Operations

The version-1 operation kinds are deliberately consumer-driven:

- callback invocation;
- return production;
- reactive read;
- reactive write;
- invalidation or refresh;
- resource creation;
- cleanup production or registration;
- disposal.

An operation contains:

- local stable ID;
- kind;
- guard;
- execution trigger and scheduling relation;
- tracking relation;
- owner relation;
- cardinality;
- input and output value shapes;
- referenced resources;
- causal dependencies;
- error and cleanup edges.

Edges have the closed vocabulary `orders`, `data`, `invalidates`, `error`,
`cleanup`, and `lifetime`. `orders` is the only pure sequencing edge. The other
edges assert the named semantic relation and may additionally imply ordering;
normalization never converts timing coincidence into causality.

The graph is acyclic. Repetition is represented by trigger/cardinality, not a
graph cycle. This keeps scheduling, repeated invocation, async emissions, and
cleanup replacement explicit without turning the contract into a full runtime
trace.

## Trigger cause and execution point

An operation records separately what makes it eligible (`trigger`) and where it
actually runs (`at`). Both use semantic events, but they are not aliases. For
example, an invalidation may trigger an effect whose apply operation executes
at `flush`; a returned cleanup may be produced at `flush` and invoked later at
`cleanup`.

The version-1 event vocabulary is:

- call;
- render;
- flush;
- settle;
- transition;
- async emission;
- cleanup;
- external event;
- request;
- response commitment.

The scheduling relation is `same stack`, `queued`, or `external`. A queued
operation names its drain event in `at`; an external operation names the
external event or request resource that invokes it. Ordering edges express
before/after constraints among actual operations. A generic `deferred` bucket
is insufficient and is not part of the normalized vocabulary.

## Tracking

Tracking is independent of scheduling and ownership:

- tracked;
- untracked;
- ambient at execution;
- unknown.

`ambient at execution` preserves a relation when the contract cannot determine
the caller's eventual tracking state. It is not normalized prematurely to
tracked or untracked.

## Ownership

Owner source is relational:

- none;
- ambient at call;
- ambient at execution;
- captured resource;
- created resource.

Owner capabilities are separate:

- child owners allowed, forbidden, or unknown;
- cleanup supported, unavailable, or unknown;
- lifetime bound to call, resource, owner, request, transition, or async source.

An operation also records owner requirements independently: required,
forbidden, or unconstrained; and, when required, the capabilities it needs.
Creating an owner does not prove the operation itself required one. Executing
under an owner does not prove that child owners or cleanup registration are
allowed.

Owner production, owner requirement, owner source, owner capability, and owner
lifetime are distinct facts. Labels such as `leaf` may be derived for
diagnostic wording but are not the semantic primitive.

## Resources

Resources correlate operations across time and branches:

- owner;
- reactive source;
- async computation;
- transition;
- cleanup;
- request;
- response;
- stream;
- server-function reference.

A resource ID is local to one normalized export summary. Summary expansion
must alpha-rename IDs before composition when necessary.

## Recursive value shapes

Required shapes:

- unknown leaf;
- plain value;
- parameter reference;
- tuple;
- array;
- object;
- choice;
- callable;
- Promise;
- AsyncIterable;
- reactive accessor;
- reactive setter;
- store;
- action;
- component;
- cleanup;
- ref application;
- server-function reference.

Projection and snapshot are represented by observable capability and
resource relationships unless RC.3 exposes a runtime-observable protocol that
requires a distinct marker. Nominal TypeScript branding is evidence for exact
identity but does not automatically create a runtime behavior category.

Every tuple item, object property, array element, and choice alternative may be
unknown independently. A missing or unknown child never contaminates known
sibling leaves.

Tuple items, object properties, and choice alternatives are themselves local
set-valued claim domains. Each composite node carries its own collection and
local closure. A closed empty property or alternative collection proves none;
a closed tuple-item collection proves the exact tuple length. Arrays carry a
leaf-local element shape plus a separately proved length interval. No parent
shape closes a descendant collection.

## Capability constraints

Normalization rejects contradictory combinations. At minimum:

- setters are writable but not implicitly readable;
- snapshots are not writable;
- optimistic state requires a writable transition-bound resource;
- refreshable values reference a refreshable source;
- pending-aware values reference an async resource;
- cleanup callables bind a cleanup resource or lifetime;
- server-function references do not imply local in-process invocation;
- plain values carry no reactive capability by default.

## Guards

Guards are conjunctions of a restricted set of atoms:

- selected signature ID;
- argument count;
- finite literal value;
- callable/value/Promise/AsyncIterable kind;
- fixed property presence or callability;
- exact tuple alternative;
- result protocol;
- exact artifact case.

General boolean expressions, regex, type-name matching, arbitrary truthiness,
user predicates, and framework labels are invalid. A complete branch partition
requires disjoint verified cases plus a verified `otherwise` for any open-ended
remainder. When a call site cannot select one case, consumers monotonically
join every possible case.

Atom order has no semantic meaning and is canonicalized by atom kind and
operand. Case order is provenance only: all non-`otherwise` cases must be
pairwise disjoint, and `otherwise` is the complement of their union. There is
at most one `otherwise`, and it is last on the wire. Negation is not a guard
operator; finite false values and the verified remainder express negative
alternatives.

## Artifact cases

An artifact case binds:

- package name, version, and integrity;
- requested entrypoint;
- exact package-export branch trace;
- runtime artifact path and digest;
- declaration target and digest;
- dependency/import closure digest;
- relevant transform identity;
- normalized export map.

Selection is exclusive. Cases are never merged. Cases with identical runtime
bytes may share a semantic surface only when declaration, dependency closure,
transform, export surface, and proof root also match.

## Stability

Experimental status attaches to the exact effective export in an artifact
case. Entrypoint status is shorthand only when every export shares it. Version
1 carries only the positive `experimental` marker: absence means unknown,
never stable. The marker requires hash-bound evidence from published package
metadata/declarations or an exact official source revision. Documentation
without exact release identity cannot certify it. Server components remain
experimental until published authority explicitly changes that status.

## Canonical semantic identity

The semantic digest includes normalized behavior and exact artifact identity.
It excludes wire schema version, formatting, key order, summary IDs, evidence
paths, and receipt bytes. It includes a separate semantic-model version so an
incompatible change cannot reuse an older digest.

Canonicalization expands summaries with alpha-renamed local IDs, sorts
semantically unordered sets, preserves ordered tuple positions and causal
ordering, normalizes guards, and hashes length-delimited typed values. It never
hashes a pretty-printed JSON representation. Unknown, partial, complete
positive, and complete negative states have distinct encodings.

## Core invariants

- Every referenced operation and resource exists.
- Operation IDs are unique after summary expansion.
- Graph ordering edges are acyclic.
- Every callback invocation references an operation.
- Every closed domain has an accepted proof.
- Every complete guard partition is disjoint and exhaustive.
- Unknown guards join monotonically.
- Recursive knowledge is leaf-local.
- Exact runtime and declaration export identities agree with the artifact case.
- Structural identity failures refuse a case; semantic incompleteness opens a
  claim domain.
- Consumers receive normalized semantics, never compact wire conventions.
