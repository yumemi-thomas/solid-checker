# Compiler facts

Solid compiler facts describe original-source JSX execution semantics. The
production Rust checker loads the controlled `solidjs-compiler`
implementation in-process, so facts come from the same transform branches as
compilation. Solid 2.0 uses the semantic-only
[`solid`](https://github.com/yumemi-thomas/solid) fork based on the official
compiler under `packages/compiler`; Solid 1.x remains on the separate
[`solid-1x-compiler`](https://github.com/yumemi-thomas/solid-1x-compiler) fork.
Both are built without their Node-API feature. The revisions are pinned in
`rust/Cargo.toml` and recorded in `THIRD_PARTY_NOTICES.md`.

## The semantic trace

The compiler emits a `SemanticTrace`: a versioned, total list of
`ExecutionSite` records pairing an original-source span with a typed semantic
decision. The compiler censuses the JSX execution sites it lowers before
lowering them, and fails closed if any censused site reaches the end without a
decision — so within the census there is no unclassified hole.

The census is *not* a census of every JSX expression in the source. Each
producer censuses the JSX it lowers, and it does not lower everything: 1.x
drops a nested non-hydratable `<head>` before recording its descendants, while
some lowering paths retract sites without replacing them with an `Elided`
decision. A source-level JSX expression a producer never censused reaches the
checker as absence, and absence is not evidence that it runs untracked or that
it was deleted. By contrast, current-pin void child lists are positive facts:
kept positions are tracked and deleted positions are one discarded range.

The trace carries a `version` field (`SEMANTIC_TRACE_VERSION`, currently 2), and
each dialect adapter refuses a trace whose version is not the one it was written
against rather than reading the fields it recognizes and assuming the rest mean
nothing. Facts may be added within a version; a removal or a meaning change is a
version bump.

That check is entirely consumer-side, and it has to be written carefully to mean
anything:

- **The adapter compares against its own literal**, `READS_TRACE_VERSION = 2`,
  declared in each of `rust/dialects/solid-v1/compiler` and
  `rust/dialects/solid-v2/compiler`. Comparing against the producer's exported
  `SEMANTIC_TRACE_VERSION` would be tautological — the producer fills the field
  from that same constant — so the runtime check could never fire for any
  producer, including a version-3 one arriving through a pin move. It was
  written that way once; the unit tests passed because they built the wrong
  version by adding to that same constant.
- **A `const _: () = assert!(…)` per adapter** holds the producer's constant
  equal to the consumer's literal, so a pin move that bumps the schema version
  fails the *build* rather than quietly making the runtime refusal unreachable
  again.
- **The producer's `#[serde(deny_unknown_fields)]` protects nobody here.** No
  code in this repository deserializes a `SemanticTrace`: `compile()` returns
  the struct in-process, and the adapters read its fields directly. The version
  gate and the compile-time assert are the whole of the consumer-side
  protection. The one remaining silent-widening path is a struct literal filled
  with `..SemanticTrace::default()`, which absorbs a newly added field without
  complaint — so both adapters' version tests name every field instead, and a
  producer that adds one fails the build.

Each dialect compiler adapter projects the trace onto the checker's
`ExecutionMap` boundary:

| Site decision | Execution map |
| --- | --- |
| `Value(ReactiveRerun)` | tracked region |
| `Value(EagerOnce)` | untracked region |
| `Value(Elided)` | discarded region |
| `Value(EagerOnce)` on a component child | deferred callback |
| `Value(CallerContext)` | deferred callback |
| `Callback(LaterEvent)` | `event-handler` callback |
| `Callback(LaterRender)` | `render` callback |
| `Callback(RefApply)` | `directive-apply` callback |

With the compiler's default effect wrapper, every `ReactiveRerun` also yields an
owned ownership region: the generated wrapper establishes a reactive owner while
that source region executes. Custom and disabled effect wrappers yield no
ownership claim, because the runtime that would establish the owner is then not
the audited one. Absence is deliberately unknown, not unowned; component,
control-flow, event, and ref ownership continues to be composed from exact
TypeFacts identity and runtime contracts.

Where that rule is applied differs by dialect, and only there. The Solid 2.0
producer still reports it as a `SemanticTrace::ownership_sites` entry and the
2.0 adapter projects those entries. The Solid 1.x producer removed that field
in version 2 in favor of additive wrapper facts, so the 1.x adapter applies
the same rule itself, over the same trace sites, using the effect-wrapper
option it already had to pass to the compiler. The resulting `ExecutionMap` is
identical either way; nothing downstream of the adapters can tell which side
derived it.

Version 2 also adds `owner_establishments`, `component_render_sites`, and
`deferred_callback_sites` — additive, span-level observations of the wrappers
lowering emitted. No adapter consumes them yet; see
`docs/precision-backlog.md` for the join rules they require and what they could
strengthen.

## Census gaps

`ExecutionMap::uncovered_jsx_expressions` holds the census against *itself*: a
site the producer censused but left unclassified. It cannot see a source-level
JSX expression the census never listed, and both producers have shapes they do
not list.

The checker used to read that absence as a proof. A reactive read inside such an
expression matched no tracked region, no untracked region, no callback role and
no JSX operation, fell through to "inside a component body, classified by
nothing", and SC1001 fired as a **proven violation** about an expression the
compiler had declined to report on.

`missing_jsx_census` in `rust/crates/solid-reactive-ir/src/execution_role.rs`
closes that. When the untracked-rendering role came from the fall-through rather
than from a fact, it finds the narrowest source-level JSX region containing the
read — an attribute expression container, a spread container, or a child, all
read from solid-facts' syntax rather than from the census, because the question
is precisely what the source has that the census does not — and asks whether any
census entry touches it. If none does, the read is reported as **uncertifiable**:
a missing compiler fact, worded as one in both the message and the evidence
chain.

Two things it deliberately does not do. It does not certify the read safe — "the
compiler deleted this expression" is a second claim with no more evidence behind
it than the first. (When the compiler *does* say it deleted the expression, that
is a discarded region, below, and it is a fact rather than a hole: silence
follows from the fact, never from the silence.) And it does not fire for a role
any *fact* established: the escalation is gated on `UntrackedRendering`, so a
read the dialect proved runs in an untracked callback keeps its proven violation
even inside an uncensused region, because that proof never came from the census.
Verified live at the current pins:
`<br>{runWithOwner(owner, () => a())}</br>` — the 2.0 census gap
of the fixture below, wrapped around a callback `Solid2` reports untracked reads
in (`reports_untracked_reads_at`, `RunWithOwner` argument 1, giving
`UntrackedCallback`) — stays an SC1001 **violation**, matching its censused
control.

A hole arrives two ways, and the mitigation cannot key on which. Either the
producer never censused the expression (1.x's nested non-hydratable `<head>`),
or it censused the expression and then **retracted** the site during lowering
without a replacement decision (the textarea `value` fold or the inert
`<noscript>` fast path). Both leave the same shape, so both take the same
wording and verdict. Current-pin void child lists are not holes: the producers
record their discarded ranges explicitly.

Fixtures: `fixtures/reactive-ir/jsx-census-gap-solid-1x` (a read in a nested
non-hydratable `<head>`, child and attribute arms) and
`fixtures/reactive-ir/jsx-census-gap-solid-2` (an inert `<noscript>` child,
censused then retracted, with a template-root void child as a positive discarded
control). Its dynamic-`textContent` arm follows Ryan's current `next` semantics.
Each fixture also pins the two negatives — a censused tracked read stays silent,
and an untracked read outside all JSX stays a proven violation.

## Discarded regions

`Value(Elided)` is the compiler reporting a deletion: the site was censused, a
decision was reached, and nothing was emitted for it. Both adapters project it
to `ExecutionMap::discarded_regions`, and the IR classifies a span inside one as
`ExecutionRole::DiscardedRendering`.

It is deliberately a category of its own, and it took a defect to establish
that. Until 2026-08-24 both adapters projected `Elided` and `EagerOnce` to the
same untracked region, so a reactive read inside a deleted value was a **proven
SC1001 violation** — "the read sees the current value once and never updates" —
about code no compiler emits. Every clause is false when the read does not
happen. `EagerOnce` keeps the untracked projection, because it evaluates exactly
once and that sentence is true of it.

Three properties define the class, and the third is what keeps it from becoming
a certification channel:

- **It is not a hole.** `census_touches` counts a discarded region, so
  `missing_jsx_census` does not escalate a deletion into an uncertifiable
  obligation. Nothing is missing: the compiler reported on this JSX and said the
  code is gone.
- **It is not an execution claim.** `reports_untracked_read` and
  `reports_disallowed_write` both exclude it, so no read, write or action
  finding is projected from it, and `contract_callback_execution` publishes no
  timing for a callback inside one — "inline" would license a consumer to run
  it eagerly, which is a positive claim dead code cannot support.
- **It proves nothing positive either.** Silence over a discarded region means
  "both compilers deleted this", never "this was proven safe". Deletion
  dominates: `execution_role` and `semantic_execution_role` both answer
  `DiscardedRendering` for a span inside a discarded region *before* consulting
  any narrower region or any semantic role, so a deleted value cannot certify a
  read through a deferred position, or convict one through an `untrack()`
  position, on the strength of syntax whose code was removed. Dominance is safe
  because every one of both producers' `Elided` spans is a single attribute or
  child *value* expression, never a wider enclosing construct — checked at both
  pins, and pinned by the adapters' own unit tests.

The deletion gate is applied once at the final common IR seam to every
source-derived diagnostic table that does not otherwise ask for an execution
role: structured static defects, upstream-compatible static violations,
directive creations, and contract-generation obligations. Thus a syntax/API
shape such as a one-argument `createEffect` inside compiler-deleted JSX is
absent, while the byte-identical live shape still reports. Individual rules do
not get to invent different dead-code policies.

At the current pins there is no checker-maintained child-content transform
divergence. Solid 1.x is intentionally faithful to its shipped Babel compiler:
its void set includes the legacy `keygen` and `menuitem` tags, and discarded
void or `<noscript>` child lists carry positive `Elided` facts. Solid 2 follows
Ryan's authoritative `next` semantics rather than treating Babel parity as its
target: nested native void children remain live and tracked, while a
template-root void child list is discarded and reported as one `Elided` range.

That distinction belongs in the producers' traces, not in a consumer-side tag
table. The former `DivergentLowering` carrier, dialect void-list hook, rerun
suppression, owner suppression, and autofix suppression were deleted after both
pins supplied truthful facts. `fixtures/reactive-ir/jsx-void-child-divergence-solid-{1x,2}`
keeps byte-identical source across the dialects and proves that the different
intentional outcomes are both certified. Missing facts still fail closed through
`missing_jsx_census`; positive deletion facts still dominate through
`DiscardedRendering`.

The historical investigations and baseline measurements for the retired
mitigations remain in `docs/precision-backlog.md`. New compiler disagreements
must be established with producer probes before adding consumer policy; compiler
silence is never enough.

`Value(CallerContext)` is the dynamic component property: the expression is
handed to the child as a getter and re-evaluated in the child's tracking
context, so it is deferred rather than untracked. A component child is invoked
from the component's own render for the same reason, even though the value
itself is built once.

The hardened DOM contract covers these compiler decisions:

- Dynamic native JSX children and attributes are tracked regions.
- Expressions the compiler evaluates exactly once are explicit untracked
  regions: template-inlined and unwrapped-insert children (including
  `staticMarker` holes), one-shot `setAttr` attribute values, and by-value
  component properties. "Once" is the claim — the code runs, at render, outside
  any tracking scope — so a reactive read there is a proven stale read.
- Expressions the compiler **deletes** are discarded regions, and they are the
  opposite claim, not a weaker one: a `Value(Elided)` value evaluates *zero*
  times. Every one of both producers' `Elided` sites is either a confidently
  foldable constant baked into the template or a value discarded unlowered (a
  `children` attribute shadowed by real children, a promoted capture the slot's
  winner drops, a spread's skipped `$key`/`children`, 1.x's shadowed component
  `children` prop) — none of them evaluates at runtime, in this producer or in
  the compiler Solid ships. See "Discarded regions" below.
- `on*` JSX values are deferred `event-handler` callbacks rather than tracked
  reads at element creation.
- Dynamic component properties and component children are deferred callbacks;
  their operations retain distinct `component-property` and `component-child`
  kinds so a component value prop is not confused with a render callback.
- `hydratable`, `dev`, `effectWrapper`, `wrapConditionals`, `staticMarker`, and
  sorted, unique `builtIns` are forwarded exactly to the compiler.
- Fact arrays are sorted deterministically by original UTF-8 byte spans.

Completeness invariant: every `jsx-expression` operation must be covered by a
tracked region, an untracked region, a discarded region, a callback role, or a
`component-property`, `component-spread`, or `component-child` operation.
Because the trace is total, every site lands in
exactly one category and the invariant holds by construction. Completeness is
a producer-integrity property: compiler-adapter tests must reject or expose an
incomplete trace before project analysis, rather than translating a producer
bug into a user-facing diagnostic.

Only DOM generation is supported. Other renderer modes, malformed options,
unknown fact kinds, invalid UTF-8 boundaries, stale hashes, and incompatible
protocol versions fail closed.

## Moving the pin

Solid 2 compiler conformance lives on the semantic-only fork branch and compares
its output with the exact `solidjs/solid` base. Solid 1.x conformance remains in
its dedicated compiler fork. To adopt new compiler work, update the appropriate
compiler `rev` in `rust/Cargo.toml`, refresh `THIRD_PARTY_NOTICES.md`, regenerate
the compiler-bootstrap conformance report, and run `make verify`.
