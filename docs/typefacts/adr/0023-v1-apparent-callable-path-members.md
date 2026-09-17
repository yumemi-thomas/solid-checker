---
status: accepted
---

# V1 separates declared callable-path members from apparent ones

## Decision

The callable-path census (`callablePaths` and `resultCallablePaths`) now
distinguishes two kinds of member and answers one shape it previously skipped.
Handshake protocol moves 10 → 11 and the schema digest moves with it.

**Declared members.** A member some declaration in the program writes down.
Every path fact carries the new required boolean `apparent`, and a declared
member carries `false`. `subtreeEnumerated` is a claim about declared members:
it says every declared member of this node's subtree was enumerated. The
whole-census closure a consumer requires
(`require_export_callable_paths_closed`, `require_all_callable_paths_closed`)
is therefore a claim about the declared-member census, and skips facts with
`apparent: true`.

**Apparent `Function` members (mechanism A).** For every walked node whose
reduced apparent type carries at least one call or construct signature — the
same predicate `callabilityOfType` asks, `GetSignaturesOfType` for
`SignatureKindCall` and `SignatureKindConstruct` — the census emits one fact
per member of the global `Function` interface that the node does not already
declare itself. The member *names* come from
`Checker_getGlobalType("Function", 0, false)`, a new `go:linkname` shim, because
`GetPropertiesOfType` never enumerates them. Each member's *type* comes from
`GetTypeOfPropertyOfType` on the node, and its *presence* from the optional flag
on `GetPropertyOfType(node, name)`, so shadowing, `strictBindCallApply`'s
`CallableFunction`/`NewableFunction` signatures, and a user augmentation of
`interface Function` — including an optional member such as `maybe?(): void` —
are all TypeScript's answer rather than a shape reconstructed here. Callability,
constructability, `complete` and `openReasons` are computed exactly as for any
node, so `prototype` and `arguments` — declared `any` — are honestly `openType`
rather than rounded up to complete.

These facts are leaves: `subtreeEnumerated: false`, no recursion, and
`apparent: true`. They are also excluded from the template set that
cross-alternative reconciliation answers for, while still serving as *prefixes*
there — which is how nothing below an apparent leaf can be proved absent.

The call/construct-signature gate in `appendApparentFunctionMembersLocked` is
the whole rule and is load-bearing:
`GetTypeOfPropertyOfType` resolves `toString` on *any* object type through the
compiler's global-`Object` fallback, so an ungated walk would decorate a plain
record with apparent members. The census carries `Function`'s augmentation only,
and only where the compiler's own `Function` fallback applies.

**Tuple members (mechanism C).** After a tuple's fixed element slots — which
remain `Tuple` path segments — `walkDeclaredMembersLocked` also enumerates the members the
tuple's `Array` or `ReadonlyArray` base declares, skipping every property whose
name is a canonical array index (`0`, `1`, …), because those slots are already
the `Tuple` segments. These are declared members, not apparent ones. An
optional or rest tuple keeps the `openIndex` it already had: a member census
says nothing about how many slots exist.

**No absence claim for an augmented name.** Cross-alternative reconciliation in
`callablePathsLocked` synthesizes a fact for a template path in every
alternative that did not observe it, and proves it `absent` when the nearest
prefix is closed and enumerated. `templateNameMayBeCompilerAugmentedLocked`
now suppresses that inference whenever the template's last segment is a
property whose name belongs to the global **`Object`** interface, resolved
through the same shim by `globalInterfaceMemberNamesLocked` and cached as a set
by `objectMemberNamesLocked`: such a name retains the explicit
`unknown` / `openAlternative` fact instead. A closed declared census is evidence
about *declared* members only, and `GetPropertiesOfType` never enumerates the
`Object` fallback.

The decision itself lives in the free function `templateNameMayBeAugmented`, so
its two guards are testable without a program. It **fails closed**: if the
global `Object` interface does not resolve at all, *every* template retains the
explicit unknown and no absence is synthesized anywhere — without the name set
there is no way to tell a genuinely absent member from one the `Object` fallback
supplies, and the census must not guess in the permissive direction. Only the
last segment is consulted, and a `Tuple` segment never suppresses: an element
slot is decided by the tuple's own shape.

The global `Function` interface is deliberately **not** consulted. Its fallback
applies only to a type with call or construct signatures, and every such
alternative emits all nine apparent leaves at the same path length, because the
depth decrement is uniform — so a `Function`-only name is reconciled into an
alternative only when that alternative genuinely lacks it, and `tsc` agrees that
`({ q: 1 }).bind` does not exist. Suppressing there would give up a correct
absence for all nine names and buy nothing.

## Why

Eight ecosystem rows refused certification with "operation value path is absent
from the signature census" for a path the compiler answers and `tsc`
type-checks:

- `@solid-primitives/i18n@2.2.1` `proxyTranslator`, in[0] `translate.bind` — a
  `Translator<T, O>` function type. `getPropertyOfType` falls back to the
  global `Function` interface for any object type with call or construct
  signatures (`getPropertyOfTypeEx`'s
  `globalCallableFunctionType`/`globalNewableFunctionType` branch), but
  `GetPropertiesOfType` does not enumerate that fallback. The census said the
  value had *no* members at all.
- `@solidjs/router@2.0.0-next.18` `action`, output `toString` — the same shape
  on a returned callable.
- `@solid-primitives/utils@7.0.0-next.4` (two rows) `wrapSetter`, in[0]
  `slice` on a `Signal<T>`/`[Store<T>, StoreSetter<T>]` tuple. The tuple branch
  enumerated only fixed elements and returned, although
  `GetPropertiesOfType(tuple)` returns `slice`, `length`, `map` and the rest.
  A plain array already censused them, so the same member was answerable for
  `T[]` and absent for `[T, T]`.

The census was also unsound in the permissive direction. A function-typed node
has no own properties, so it was marked `subtreeEnumerated: true`, and
`callablePathPrefixProvesAbsence` would then let it prove a sibling
alternative's `f.bind.x` *absent* — a member the compiler answers. Emitting the
apparent leaves closes that: a template of path length L can only exist when
every alternative was walked to length L (`remaining` decrements uniformly), so
the nearest prefix of `f.bind.x` is now the apparent `f.bind` leaf, whose
`subtreeEnumerated: false` proves nothing below itself.

Recursion into apparent members is deliberately refused rather than bounded.
`bind.bind.bind…` is unbounded in the useless direction, and the values reached
are library-owned and never caller-supplied, so no proof about the package
under analysis can rest on them. That is also why whole-census closure excludes
them: two of the nine, `prototype` and `arguments`, are declared `any`, so
requiring them to be closed would refuse every callable value on the strength
of a `Function.prototype` no package ever wrote.

### Why absence may not be inferred for an augmented name

Mechanism C made this reachable and an adversarial review confirmed it. For
`[Accessor<number>, Setter<number>] | { dispose(): void }`, the tuple
alternative's Array members produce `toString` and `toLocaleString` templates;
the `{ dispose(): void }` alternative is closed and fully enumerated and names
neither, so both were synthesized `absent` — while `tsc` accepts
`({ dispose(): void }).toString()`. Symmetrically, an alternative declaring
`hasOwnProperty` or `valueOf` made a sibling `() => void` alternative
`absent` for them, although the compiler answers both through `Object`. The
same hole predates the tuple census for any alternative that declares an
`Object` member outright (`{ toString(): string } | { q: number }`), so the
fix covers declared templates too.

An earlier draft also suppressed the global `Function` interface's names and
dropped the "apparent facts do not become templates" exclusion. Both were
reverted after measurement.

Suppressing `Function`'s names gave up correct absences for all nine of them —
`tsc` rejects `({ q: 1 }).bind` — and bought nothing, because the uniform depth
decrement means any alternative the `Function` fallback applies to has already
emitted the leaf itself.

Dropping the exclusion cost closure outright. The facts reconciliation
synthesizes carry `apparent: false`, so a consumer counts them as declared
census members; every one of them is an explicit unknown; and
`require_all_callable_paths_closed` therefore refuses. `(() => void) | undefined`
at depth 1 measured 20 facts with **9 open declared facts** that way, against
11 facts and none open with the exclusion in place. The templates that close
the false-absence hole all come from *declared* siblings, so they are
unaffected by the exclusion.

### Nested union member presence: investigated and withdrawn

A fourth mechanism was implemented and then removed. It answered per-member
presence for a union reached below the root, because
`getPropertiesOfUnionOrIntersectionType` returns only the members common to
every constituent and stops at the first constituent without an index
signature, so `Set<T> | undefined | null | false` enumerated nothing at all.
It would have unlocked `@solid-primitives/keyed`'s
`SetValues` in[0] `of.values`.

It was falsified against the real published typings. The rule certified accesses
TypeScript rejects:

- `{ handle: { dispose: () => void } | undefined }` produced `handle.dispose` as
  optional / callable / complete, while `tsc` reports
  **TS18048: 'handle' is possibly 'undefined'**.
- `{ of: { run: () => void } | { other: 1 } }` produced `of.run` as optional /
  callable / complete, while `tsc` reports
  **TS2339: Property 'run' does not exist on type '{ run: () => void; } | { other: 1; }'**.

The `ReadPartial` filter in `getPropertyOfUnionOrIntersectionType` is
TypeScript's *answer* that the member is not accessible on the union, not an
enumeration gap to be worked around. The asymmetry the review found makes it
plain: `{ run?: () => void }` censuses as optional / **mixed** and certifies
nothing, because the optional member's type folds in `undefined`; the union
spelling of the same value was reported optional / **callable** and accepted.
Any faithful model of a partially-present union member has to reach the same
`mixed` answer, which certifies nothing — so the mechanism unlocks no row even
when it is honest. It also created the false-absence templates described above
and unlocked zero rows in practice, because the `keyed` rows advanced only to
an unrelated `operation-cardinality` frontier. Nested unions therefore keep
their pre-change behaviour, and `of.values` remains an exact fail-closed
refusal — including the pre-existing `subtreeEnumerated` over-claim on such a
node, recorded under the approximations below.

## Consequences

Protocol 11 is a break in both directions, not an additive extension. A
protocol-10 consumer would count an apparent leaf as a declared census member
and could read a library-owned `bind` or `prototype` as part of the package's
own surface; a protocol-10 producer omits the field this protocol requires.
`apparent` therefore has no `serde(default)` on the Rust side and no
`omitempty` on the Go side: missing wire data is rejected rather than
defaulted. The Rust client additionally refuses an apparent fact that claims
absence, an enumerated subtree, or the root, because any of those would let a
library-owned leaf close a census or prove a path below itself absent.

Exact path lookups (`require_operation_recursive_signature`,
`require_export_recursive_subject`, callback-parameter path matching) are
unchanged and find the new facts. `require_verifiable_root_premise`,
`require_root_callability` and `require_path_callability` are unchanged.

### Cost

The census grows sharply in the shapes mechanism C reaches, because a tuple
brings in roughly 35 `Array` members and mechanism A then decorates each
function-typed one with nine leaves. Measured on
`[Accessor<number>, Setter<number>]` as a first parameter:

| demand depth | total facts | of which apparent | pre-change |
| --- | --- | --- | --- |
| 1 | 42 | 0 | 3 |
| 2 | 408 | 360 | 3 |
| 3 | 462 | 414 | 3 |

Row timings are much milder — `solid-js@1.9.14|solid1|only` went 19149 ms →
16377 ms wall and 6687 ms → 6056 ms certification, and
`@solid-primitives/utils@7.0.0-next.4|solid2|head` 2707 ms → 2814 ms wall and
1323 ms → 1444 ms certification — but those are **measurements of two rows, not
a bound**. A demand's census depth is the maximum path length over all demands
for the same export, so an export that needs a depth-2 path pays the depth-2
table above. The full-corpus re-measurement is what will give the real cost.

### Known approximations

- The census carries the global `Function` interface's apparent members but
  **not** the global `Object` interface's, which `getPropertyOfType` also falls
  back to for every object type. `Object`'s names are used to *suppress* an
  absence claim but are never emitted as leaves, so an exact-path demand for
  `x.hasOwnProperty` still fails closed on a missing fact. Censusing them was
  rejected on growth: the table above would grow again and no row needs it.
- A package's own `declare global { interface Function { $x: any } }`
  augmentation is emitted as an *apparent* leaf, and is therefore skipped by
  declared-census closure even though the package under analysis did write it
  down. That is not a regression — before this change it was not censused at
  all — but it is a case where "apparent" and "library-owned" come apart.
- At the demand's depth cut a callable node reports `subtreeEnumerated: true`
  when it declares no members of its own, while its apparent leaves were not
  emitted (they appear only at nodes with remaining depth ≥ 1). This is correct
  under the declared-member reading of `subtreeEnumerated`, and it cannot feed
  an absence proof: a template one segment deeper would require one more level
  of depth in every alternative.
- **Open: nested union nodes over-claim `subtreeEnumerated`.** A union reached
  below the root reports its declared subtree as enumerated even when
  `GetPropertiesOfType` dropped a constituent's members, so an absence can be
  synthesized over a real callback. Reproduction: for
  `{ foo: { bar: () => void } } | { foo: { bar: () => void } | { baz: 1 } }` at
  depth 3, the second alternative's `foo` is
  `required / complete / subtreeEnumerated: true` with no members, and
  `foo.bar` is synthesized `absent` — while the value may well carry `bar`. A
  whole-census closure can therefore succeed over a hidden callback. The sound
  fix is for such a node to report `subtreeEnumerated: false` with an open
  reason, but that refuses closure for every `T | undefined` member and needs
  its own measured slice. Recorded in `docs/precision-backlog.md`.
