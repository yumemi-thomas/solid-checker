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
- resource registration (`create` — see § "What a closed call domain denies"
  § creates for what it does and does not cover; declaring a resource in
  `call.resources` is not this operation);
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

## What a closed call domain denies

`call` carries nine set-valued claim domains: `callbacks`, `reads`, `writes`,
`creates`, `invalidates`, `throws`, `returns`, `cleanups`, `disposals`. Naming
one in `closed` beside an empty collection is a *complete negative* claim about
one invocation of one export, in one artifact case, under one guard. This
section states what each such claim denies.

The scope is *one invocation*, so a resource established while a dependency's
module initializes — the module-level `createSignal` a package runs on first
import, or a root a dependency opens at load — is outside every domain here: it
is not caused by this invocation, and closing a domain says nothing about it.

It fixes meaning that was previously implicit in the audited documents and the
generator, and the two disagreed — see
`phase21/2026-09-03-implementation-census-plan.md` for the reconciliation and
its consequences. Nothing here changes an encoding, a key, a canonical stream,
or a digest: `semanticModelVersion` stays 1.

Four rules hold for every domain and are stated once.

**Kind.** Eight of the nine domains admit exactly one operation kind, and the
mapping is enforced in both directions by `validate_call_claims`
(`rust/crates/solid-reactive-ir/src/contract_semantics/validate.rs:1030-1128`):
a domain accepts only its kind, and every published operation must appear in
its kind's own domain or the document is a contradiction. `throws` is the
exception — it constrains no kind — so an operation named there is *also* named
in its kind's domain, and `throws` is a second label on an existing operation
rather than a domain with operations of its own.

**A caller-supplied callable's body is not this export's behavior.** An
operation reachable only through a callable the export invokes belongs to this
export's domains when the export itself supplies that callable, and does not
belong when the callable is caller-supplied — reached from a parameter, or from
a value the caller handed over. The export's own act, the invocation, is the
`callbacks` item; what the caller's function does inside it is the caller's
behavior. `createEffect` is the audited case: `reads: []` is closed while
`initial-compute` is `tracking: tracked`, which is coherent only because the
reads the caller's computation performs are not `createEffect`'s
(`pkg/contracts/bundled/solid-v2/solid-js.json`). What does cross the boundary
is *registration*: `onSettled`'s `returned-cleanup` and `createEffect`'s
`replace-cleanup` are the export registering a cleanup the caller's function
returned, and both are published in `cleanups`.

**Later execution is an operation, not an absence.** An operation caused by
this invocation belongs to its domain whenever it runs, recorded through the
separate `trigger` and `at` fields, so a closed domain denies the
later-scheduled operation exactly as it denies the same-stack one. Audited:
`createEffect`'s `queued-apply` (`at: {event: flush, schedule: queued}`) and
`repeated-compute` (`at: {event: external-event, schedule: same-stack}` — the
event is later, the run is on that event's own stack) are `callbacks` items;
`render`'s `delegated-event` (`at: {event: external-event, schedule:
external}`) is the audited `schedule: external` case; and `render`'s
`unregister-root` (`at: {event: cleanup, schedule: same-stack}`) is a
`disposals` item (`pkg/contracts/bundled/solid-v2/solidjs-web.json`).

**Every non-call form list below is open-ended, and closure is not a licence to
read one as exhaustive.** Each list names the forms known to reach the domain
today; nothing here asserts there is no other form. A census may therefore
conclude closure only where the producer classified *every* invoking or
observing form it walked, and must refuse **by name** on any form it does not
classify — a form absent from both the producer's classifier and this document
is a refusal, never a silent pass. See the census predicate in
`phase21/2026-09-03-implementation-census-plan.md` § 3.

A sentence marked **[Decision YYYY-MM-DD]** settles a question the existing
documents left silent; the rest cite the document that already fixed it. A
later decision may *apply* an earlier one to a case it did not name, and says so
rather than restating it — § creates' 2026-09-04 paragraph is that shape.

### callbacks

`callbacks: [] closed` denies that one invocation of this export gives rise to
any operation of kind X, where X is exactly *the export invoking a callable it
did not itself define* — `kind: "invoke"`, named from the argument slot,
operation output, or summary resource the callable arrived through — including
an invocation the export performs from inside a callable it supplies itself and
one this call schedules to a later `at` event, excluding everything the invoked
callable's own body does, and arising from a call or `new` and additionally
from these non-call forms — an open-ended list, per the fourth shared rule: a
tagged template, a getter or setter reached by property access, an
iteration-protocol member reached by `for…of`, `for await…of`, spread, array
destructuring, or `yield*`, a decorator application, `then` on a supplied
thenable reached by `await`, `Symbol.dispose` or `Symbol.asyncDispose` reached
by `using` or `await using`, `Symbol.hasInstance` reached by `instanceof`, a
coercion reaching `Symbol.toPrimitive`, `valueOf`, or `toString`, a `Proxy`
trap, and a JSX element or `html` template whose compiler lowering invokes a
component or accessor.

Kind and source vocabulary: `validate.rs:1036-1056`. Trigger/`at` and the
caller-supplied boundary: `solid-js.json`'s `createEffect`, and `render`'s
`delegated-event`, whose `from` is a resource path rather than a parameter.
**[Decision 2026-09-03]** the non-call list: the producer's *call* census
records `ast.IsCallExpression` and `ast.IsNewExpression` only
(`implementationCallCensusLocked`,
`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go`), so
every form listed here — and every form the list has not yet named — must reach
a census as an explicit marker that refuses, never as silence.

**[Landed 2026-09-03, producer half]** that marker exists:
`ExportImplementationTranscript.uncensusedInvokingForms` (handshake protocol
14) carries one row per invoking form the call census does not record, over
twelve closed kinds whose **default is refusal** — a node kind that is neither
classified nor on the producer's reviewed list of kinds that provably cannot
invoke user code arrives as `unclassified-invoking-form` with the compiler's own
node-kind name. Two forms are absent by decision: a `Proxy` trap is a property
of the runtime value rather than of syntax and is outside any producer census,
and `f?.(x)` is already a `CallExpression`. No consumer reads the field yet;
`docs/typefacts/adr/0026-v1-uncensused-invoking-forms-and-local-declaration-transcripts.md`
carries the vocabulary and the limits, including why an *absent* field is not
the same fact as a present empty one.

### reads

`reads: [] closed` denies that one invocation of this export gives rise to any
operation of kind X, where X is exactly *an observation of a reactive source's
current value* — `kind: "read"`, carrying its own `tracking` relation, so an
`untracked` or `ambient-at-execution` read is still a read and only the
dependency it registers differs — including a read this call schedules to a
later `at` event, excluding a read a caller-supplied callable performs, and
arising from non-call syntax: a property access on a store, props, or
projection proxy, a whole-object observation (`$TRACK`, spread, `Object.keys`,
`in`, iterating a store), and a JSX attribute or child expression whose
lowering reads an accessor.

`latest` publishes one `read` with `tracking: untracked` and closes every
sibling domain; `createStore` publishes three; `Loading` and `isPending` leave
`reads` *open* with one item, which is partial positive and not closure.
**[Decision 2026-09-03]** the proxy property access is the load-bearing
non-call form, and it is the reason a `reads` census can never be a census of
calls alone.

**[Decision 2026-09-10] The proxy must be one the export owns.** A property
access on a store, props, or projection proxy is this export's read only when
the proxy is a value the export created or imported. An access on a
**parameter-rooted** receiver — component props, a store the caller passed — is
the *caller's* read and belongs to the caller's contract, on exactly the
argument [ADR 0034](../adr/0034-parameter-rooted-accessor-disposition.md) makes
for a getter and this section already makes for a callable:

> Code the caller attached to an object it passed is not this export's
> registration any more than a callback it passed is.

So the exclusion above reads in full: *excluding a read a caller-supplied
callable performs, **and excluding a property access whose receiver the caller
supplied***. Three things follow, and they are why the carve-out is stated
rather than left implicit:

- Every `read` operation in every bundled document is already of the owned
  kind — a reactive-resource read, never a parameter path — so the audits
  conform as written, and `Show`'s `reads: []` beside its
  `guard: {arg: 0, path: ["keyed"]}` is correct rather than contradictory.
- The literal reading was not implementable. The Type Facts producer states,
  in `invocation.go:629` and `invocation.rs:815`, that a `Proxy` trap "is out
  of the producer's reach entirely" because `obj.x` on a proxy is the same
  `PropertyAccessExpression` as `obj.x` on a plain object; and Solid types a
  store as its plain object type, so no declared-signature premise can
  separate them. A census obliged to prove "no proxy access occurred" on a
  caller's value would refuse every export that touches a parameter's
  properties, permanently.
- The census therefore reuses the derivations it already has: ADR 0034/0040
  disposition the parameter-rooted receiver, and
  [ADR 0044](../adr/0044-a-value-this-program-built.md)'s "a value this program
  built" decides the owned side — an object literal's members are data
  properties the specification created, so accessing them is not a read, while
  a proxy the export obtained by calling a Solid primitive arrives as a *call*
  the census already enumerates.

**[Decision 2026-09-10] The exclusion is about authorship, not timing.** A read
performed by a callable this export did not author is not this export's read,
whoever supplied that callable. The clause above already says so for a
*caller-supplied* one; a scheduling primitive that drains a queue —
`@solidjs/signals`' `flush` and `action`, and `@solidjs/web`'s `render` and
`hydrate` through them — runs computations a **third party** registered, which
is further from its own act than a caller's callback is. Those reads belong to
the contracts of whoever registered them.

This does not weaken "including a read this call schedules to a later `at`
event": that clause is about *when* a read this export authored happens, and it
still binds. `createEffect`'s own audited closure is the precedent — `reads: []`
while its `initial-compute` is `tracking: tracked`, which
`phase21/2026-09-03-implementation-census-plan.md` § 3.2 says "is coherent only
under this rule". A read of a source the export *created* is unaffected and
still counts, which is why `createEffect`'s row is withheld: under
`hydrating ∧ ssrSource === "client"` its client build reads a signal
`withHydrationGate` created.

### writes

`writes: [] closed` denies that one invocation of this export gives rise to any
operation of kind X, where X is exactly *a mutation of a reactive source's
value that other reactive readers can observe* — `kind: "write"` — including a
write this call schedules to a later `at` event or performs under a transition,
excluding a write a caller-supplied callable performs, and arising from
non-call syntax: an assignment, compound assignment, `++`/`--`, or `delete` on
a store or mutable proxy path.

`createStore`, `createOptimistic`, `createOptimisticStore`, `reconcile`,
`action`, and `flush` each publish exactly one closed `write`
(`pkg/contracts/bundled/solid-v2/solidjs-signals.json`). **[Decision
2026-09-03]** the assignment forms.

### creates

`creates: [] closed` denies that one invocation of this export gives rise to
any operation of kind X, where X is exactly *this export performing a published
`create` operation* — `kind: "create"`, whose own `resources` list names what
was registered, drawn from the version-1 resource vocabulary. **[Decision
2026-09-03]** `creates` is defined over the `create` **operation**, not over
the intuitive notion of a thing beginning to exist, and the operation is the
one the audits use for exactly one act: **registering a version-1 resource into
a runtime outside this invocation** — a browser document or a server runtime —
so that the resource remains live there after the call returns, reachable by
that runtime rather than only through a value the call handed back. Two
qualifications are load-bearing: the registered thing must be a *version-1
resource kind*, and the registry must be a *runtime that acts on it*. A
package's own private module variable is neither, so an export that parks an
ad-hoc object in a module-level binding performs no `create` (see
`fixtures/package-contracts/closed-domain-probe-gate`'s `runCreatingOwner`,
and § 3.3 of the census plan). The audited instances are the whole extension in
the corpus today: `render`'s `register-delegation` registers a `browser-root`
on the document, and `createServerReference`'s `register-reference` and
`transform-reference` register a `server-reference` with the server runtime
(`pkg/contracts/bundled/solid-v2/solidjs-web.json` and its
`--server-functions-*` siblings). The claim includes such a registration
performed at a later `at` event because of this call, excludes anything a
caller-supplied callable registers, and arises from these non-call forms —
open-ended, per the fourth shared rule, and in every case known today a
compiler lowering inserting the registering call: a JSX element or `html`
template that registers a row or component owner with the runtime, and a
`"use server"` function whose transform yields a server-function reference.

**[Decision 2026-09-04] Handing a value to a per-request render context that
writes it into the response is a `create`.** The question was
`solid-js@2.0.0-rc.3`'s server condition: `dist/server.js`'s `processResult`
executes `ctx.serialize(id, deferred.promise, deferStream)`, where `ctx` is
`sharedConfig.context` — the per-request context `@solidjs/web`'s
`renderToStream` installs (`@solidjs/web/dist/server.js:1325`), whose
`serialize` (`:1383-1397`) adds the promise to `blockingPromises` and chains
`serializer.write` into the response stream. That **is** the one act this
domain is about, on all four of the definition's terms: it is *the export's own
act*, not a caller-supplied callable's; the registered thing is a version-1
resource kind (the memo's pending result — an `async-computation` — landing in a
`stream`/response); the registry is a runtime that acts on it, because the
hydration serializer later writes the resolved value into the HTML the server
sends; and it remains live after the call returns, reached by that runtime
rather than through the tuple the call handed back. The 2026-09-03 sentence
above is unchanged and this is its application, not an extension: a per-request
render context is *not* the "package's own private module variable" that
sentence excludes — a module-level binding nothing outside the invocation reads
is still not a `create`, and the line between the two is whether some runtime
outside the invocation acts on what was registered.

**A guarded reach still counts, and a flat row must therefore withhold.** The
reach above is conditional — the `node`/`worker`/`deno` condition, an `async`
context, `options.ssrSource ∈ {server, hybrid}`, a thenable or async-iterable
compute result, an owner with an id, and no `NoHydrate` ancestor — and a
condition on *when* an operation happens has never made it absent: that is the
"later execution is an operation, not an absence" rule, applied to a guard
instead of to a schedule. A closure is over one artifact case under one guard,
so a *document* can close `creates: []` for the browser case and publish the
operation for the server case. A negative-authority row keyed only by
`(package, export, domain)` cannot: it has nowhere to put the condition, so an
export with any guarded reach must be **withheld** from such a table rather
than qualified. Recorded consequence, 2026-09-04: this withdrew
`solid-dialect`'s `(solid-js, createEffect, Creates)` row and withheld
`(solid-js, createSignal, Creates)`, both for exactly that guard — see
`docs/adr/0007-census-dialect-axiom-tier.md` and
`docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md`
§ 7.4. Signed off by delegation, 2026-09-04.

Three things are **not** `creates` items, and each has its own home:

- **A summary's `resources` declaration.** Declaring a resource in
  `call.resources` names something the summary's operations correlate over; it
  is not an operation at all, so it can never be a `creates` item and it never
  contradicts `creates: []`. This is what the whole audited corpus does:
  `createStore` declares a `reactive-source`, `createMemo` an
  `async-computation`, `action` a `transition`, `createTrackedEffect` and
  `onSettled` an `owner`, and `createEffect` both an `owner` and a `cleanup` —
  and every one of them closes `creates: []`.
- **Owner production**, which is the `owner.productions` domain of whichever
  operation runs under the produced owner, with its own local closure.
- **Owner requirement**, which is the `owner.requires` / `requiresChildren` /
  `requiresCleanup` triple of whichever operation needs the owner.

**[Decision 2026-09-03]** a `create` operation that names no resource is
therefore invalid. That is the shape the generator *used to* emit for an owner
requirement; it registers nothing, and it is the whole of the disagreement this
section settles. The generator no longer emits any `create` at all — see the
model gap below — but the *validation* rule that would reject one is **not yet
in force**, because two bundled documents and one fixture contract still carry
one: the fixture's is refused before decode by its catalog status, and the two
bundled documents are frozen Phase 14 authority whose correction is an
authority re-capture blocked on the census itself. The rule and those
corrections land together; the blocker is recorded with its evidence in
`docs/precision-backlog.md` (2026-09-03, "The generator stopped publishing an
owner requirement as a resourceless `create`"). Until then a document carrying
the shape decodes, and this section states what it means: nothing that
`creates` can express.

**[Decision 2026-09-03] The model gap this exposes: a free-standing owner
requirement has no domain.** An export that must be called under an ambient
owner *because it registers a computation on that owner* — the analyzer's
`OwnerRequirementOperation::Effect`, the fact behind `SC4001` — is not a
`create`: it registers nothing into a runtime outside the invocation, and it is
exactly what the audits publish beside `creates: []` **closed**
(`createTrackedEffect`, `For`/`Show`'s children, `createSubRoot`'s callback).
The three "not a `creates` item" homes above do not carry it either: the
`requires`/`requiresChildren`/`requiresCleanup` triple is a *field of whichever
operation needs the owner*, so a requirement with no operation to hang it on
has nowhere to go, and no audited document records a consumer-level owner
requirement anywhere. Version 1 therefore has no domain for it, and the
generator **withholds** it by name rather than widening `creates` — the
withholding is recorded per export, with its role and reason, in the
generator's proposal refusal sidecar (`writeProposalRefusalAudit`'s
`withheldClaims`, fed by the emitter's
`solid-checker:withheld-owner-requirement=` record). The repair is a new domain
(option (c) of the census plan's § 2.2 item 1), not a wider `creates`.

What that costs, exactly: a *generated* proposal no longer carries the
`Effect` owner-requirement positive fact, so a consumer `SC4001` derived from a
**generated** dependency contract is unavailable until the new domain exists.
Nothing live changes today — no generated contract is accepted anywhere — and
the hand-audited path is untouched: the two frozen Solid 1.x authority
documents still carry their `ambient-at-call` `create`, and
`project_owner_requirements` still reads it.

A **cleanup**-role requirement does have a home, and it is the `cleanups`
domain: the export installs a cleanup on the caller's owner, so it publishes
`kind: cleanup` with `source: ambient-at-call`, `requires: required`,
`requiresCleanup: required`. **There is no audited precedent for that shape.**
Every `kind: cleanup` operation in the bundled corpus — `solid-js`'s
`replace-cleanup`, `@solidjs/signals`'s `returned-cleanup`, `@solidjs/web`'s
`ref-cleanup`, `--web-node-server`'s `retract-declaration` — is
`requires: forbidden`, `source: none`, because each describes a cleanup the
*runtime* runs rather than one the export installs on its caller's owner. The
shape is chosen for the fact, not copied from an audit: the requirement is a
`Requirement` triple on the operation that needs the owner, the installing act
is a cleanup, and the archive's own `onCleanup` call is the witness. It names
**no resource**, as `returned-cleanup` and `ref-cleanup` also do not: a
resource declaration is a positive fact of its own with no witness on any axis
today.

Where the audits put a created owner is **not** uniform, and the model does not
pretend otherwise. `render` is the single audited `create` that also names an
owner — `register-delegation` is a `creates` item *and* carries
`owner.source: "created"` with `productions: [browser-root]`. Elsewhere an
`owner.source: "created"` with a nonempty `productions` sits on an `invoke`
that is a `callbacks` item, beside `creates: []` closed: `createTrackedEffect`'s
`callback`, `For`/`Match`/`Repeat`/`Show`'s `accessor-child` and `raw-child`,
`render`'s and `hydrate`'s own render callbacks, and the generated
`createSubRoot` family's `callback-0`. And in the remaining cases the owner's
coming-into-existence is recorded **nowhere**: `createEffect` declares
`effect-owner` but no operation of its summary carries `source: created` (only
`captured`, `ambient-at-call`, `ambient-at-execution`, and `none`); `onSettled`
declares `onSettled-leaf-owner` the same way; `@solidjs/signals` has exactly
one `source: created` operation in the entire document, `createTrackedEffect`'s;
and all fourteen audited `solid-v1/solid-*.json` documents carry no `owner`
field on any operation and no `resources` entry at all, while closing
`creates: []` for `createRoot`, `createSignal`, `onCleanup`, and every other
1.x primitive. **That is a real model gap, not a defect of this predicate**:
version 1 has no domain in which "this reactive-graph resource began to exist
on this call" is a positive, closable fact, and it is recorded as such in
`docs/precision-backlog.md`. Do not repair it by widening `creates`.

### invalidates

`invalidates: [] closed` denies that one invocation of this export gives rise
to any operation of kind X, where X is exactly *marking a reactive source or
async computation stale so its dependents recompute or refetch* —
`kind: "invalidate"`, a fact distinct both from the write that may cause it and
from the recomputation it schedules — including an invalidation this call
schedules to a later `at` event, excluding one a caller-supplied callable
performs, and arising from non-call syntax: the same store-proxy assignment
forms `writes` names, because a store write invalidates the paths it touches.

`refresh` and `affects` publish exactly one closed `invalidate` each, `action`
one, and `flush` two. **[Decision 2026-09-03]** the store-assignment form, and
that a `write` never implies its `invalidate`: the two domains are closed
independently and `createStore` closes one positive `write` beside
`invalidates: []`.

### throws

`throws: [] closed` denies that one invocation of this export gives rise to any
operation of kind X, where X is exactly *none of the eight version-1 kinds* —
`throws` is the one domain `validate_call_claims` constrains to no kind
(`validate.rs:1082-1087`), so its items are operations already published under
their own kind and additionally labelled as able to complete abruptly — and the
claim denies that any operation of this invocation, same-stack or
later-scheduled, including the export's own registration of a caller-supplied
callable but not that callable's body, propagates an exception out of the
export; no non-call syntax list bounds it, because every expression form can
throw.

**[Decision 2026-09-03]** version 1 has no `throw` operation kind, and no
audited or generated document in this repository carries a positive `throws`
item. Nor is closure the usual state: of the 119 summaries in
`pkg/contracts/bundled/**`, exactly **19** name `throws` in `closed` beside an
empty list, and the other **100** omit the field entirely, which is `Unknown` —
every solid-v1 summary and eleven solid-v2 ones. A positive `throws` claim is
therefore expressible only as a second label on an operation of another kind,
and closure is the only state the domain has ever *held* where it is stated at
all. That is a model gap
recorded in `docs/precision-backlog.md`; do not invent a ninth kind inside
version 1, and do not read the absence of positive items as evidence that
nothing throws.

### returns

`returns: [] closed` denies that one invocation of this export gives rise to
any operation of kind X, where X is exactly *the export yielding a value to its
caller* — `kind: "return"`, carrying the returned value's `output` shape, one
operation per distinct returned shape rather than one per completion, so a
repeated emission is a single `return` with `count: {scope: "call", max:
"many"}` — including a value delivered at a later `at` event, excluding a value
a caller-supplied callable returns (which reaches `cleanups` when the export
registers it, as `onSettled`'s `returned-cleanup` does), and arising from
non-call syntax: a `return` statement carrying an expression, an arrow's
expression body, a generator's `yield` and its completion value, and an `async`
function's resolved value.

**[Decision 2026-09-03]** a **valueless completion is not a `return`
operation.** A bare `return;`, a function that falls off its end, and a `void`
export all complete without yielding a value to the caller, and none of them
publishes a `return`. This is what keeps the section self-consistent: a void
export closes `returns: []` — `createEffect` and every Solid 2.0 primitive that
returns nothing do — which is only coherent if the implicit `undefined` such an
export completes with is not itself a returned value. It also fixes what a
`returns` census must decide: not "does control leave this function", which is
always true, but "does any completion carry a value", which is
control-flow-decidable from the same transcript.

`createMemo`'s `emission` is the audited repeated-return case (`count:
{scope: "call", min: 0, max: "many"}`, `at: {event: async-emission}`);
`createSubRoot`, `createSelector`, and `memo` are the ordinary ones.
`clientOnly` publishes two `return` operations but leaves `returns` *open*,
which is partial positive and not closure.

### cleanups

`cleanups: [] closed` denies that one invocation of this export gives rise to
any operation of kind X, where X is exactly *the export producing or
registering a cleanup* — `kind: "cleanup"`, binding a `cleanup` resource or a
lifetime — including a registration that happens at a later `at` event
(`createEffect`'s `replace-cleanup` is triggered by `queued-apply` and runs at
`cleanup`) and including the registration of a function a caller-supplied
callable returned, because the registration is the export's act even though the
function is the caller's; excluding the running of that function's body; and
arising from these non-call forms — open-ended, per the fourth shared rule: a
compiler lowering that inserts a cleanup at a JSX boundary, and a `using` or
`await using` declaration whose scope exit is the registration point. Every
other audited cleanup is registered by a call or by the export reading a
callable out of a value it was handed.

`onSettled` and `applyRef` leave `cleanups` *open* with one item precisely
because the caller's function decides whether one exists; `createEffect` and
`createTrackedEffect` close it with one.

### disposals

`disposals: [] closed` denies that one invocation of this export gives rise to
any operation of kind X, where X is exactly *the export ending a named
resource's lifetime* — `kind: "dispose"`, whose own `resources` list names what
is disposed — including a disposal that runs at a later `at` event
(`render`'s `unregister-root` and `createEffect`'s `dispose-effect` both run at
`cleanup`; `createMemo`'s `cancel` cancels its async computation), excluding a
disposal a caller-supplied callable performs, and arising from these non-call
forms — open-ended, per the fourth shared rule: a `using` or `await using`
declaration going out of scope, which invokes `Symbol.dispose` or
`Symbol.asyncDispose` on the declared value, and a compiler-inserted boundary
teardown. Every other audited disposal is effected by a call or by the runtime
draining an owner.

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

Semantic-model version 1 freezes the hash family as SHA-256 and the domain
separator as `solid-checker:normalized-package-contract`. Text and byte strings
are prefixed by an unsigned 64-bit big-endian byte length; sequences are
prefixed by an unsigned 64-bit big-endian item count; fixed-width integers are
big-endian; options, variants, booleans, and the four knowledge states carry
explicit tags. The canonical stream begins with the length-delimited domain,
the unsigned 16-bit semantic-model version, exact package identity, and the
ordered normalized artifact cases. The digest is rendered as lowercase
`sha256:<64 hexadecimal digits>`.

The checked golden proposal in `solid-reactive-ir` hashes to
`sha256:23c3aef34b18c809cbfe185cb53ed4b37275ab6486da190b37f4e18d8291c2b9`.
Changing any rule in this paragraph is an incompatible semantic-model change;
wire-schema numbering alone never changes this digest.

### Digest families

Two optional fields joined normalized meaning after version 1 froze:
`composedFrom` on a `read` operation, and `proposedClosures` on a `call`. Each
is written into the canonical stream only in the families that carry it, and
each family is named by its own domain separator, so a contract carrying
neither hashes exactly the bytes it hashed before either field existed — and
keeps every receipt already issued for it. The four separators are the base
string above, plus the suffix `:composed-provenance`, plus the suffix
`:proposed-closure`, plus both suffixes in that order. The family is a function
of the contract, never a mode a caller selects, and the domain is the
length-delimited first thing written, so two families cannot collide. Each has
its own frozen vector in `contract_semantics::tests`.

A document that states provenance, or that labels a closure as proposed, is a
new document making a new claim; it belongs in its own family rather than
sharing an identity with the document that makes the plainer claim.

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
