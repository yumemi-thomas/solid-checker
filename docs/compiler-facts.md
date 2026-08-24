# Compiler facts

Solid compiler facts describe original-source JSX execution semantics. The
production Rust checker loads the controlled `dom-expressions-compiler`
implementation in-process, so facts come from the same transform branches as
compilation. Solid 2.0 and Solid 1.x use separate pinned Cargo git dependencies
from [`dom-expressions`](https://github.com/yumemi-thomas/dom-expressions) and
[`solid-1x-compiler`](https://github.com/yumemi-thomas/solid-1x-compiler), both
built without their Node-API feature. The revisions are pinned in
`rust/Cargo.toml` and recorded in `THIRD_PARTY_NOTICES.md`.

## The semantic trace

The compiler emits a `SemanticTrace`: a versioned, total list of
`ExecutionSite` records pairing an original-source span with a typed semantic
decision. The compiler censuses the JSX execution sites it lowers before
lowering them, and fails closed if any censused site reaches the end without a
decision — so within the census there is no unclassified hole.

The census is *not* a census of the JSX the source contains. Each producer
censuses the JSX it lowers, and it does not lower everything: 1.x drops a
nested non-hydratable `<head>`, and 2.0 emits no site for a void element's
children **in the template-root position**, where `lower_dom_element` gates the
child pass on `!is_void_element`. Nested, both producers lower those children and
census them — which is a divergence rather than a gap, and the distinction is the
whole of "Census gaps" versus "Divergent lowering" below. A source-level JSX
expression a producer never censused reaches the checker as absence, and absence
is indistinguishable from "the compiler proved this never re-runs".

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
| `Value(EagerOnce)`, `Value(Elided)` | untracked region |
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
it than the first. And it does not fire for a role any *fact* established: the
escalation is gated on `UntrackedRendering`, so a read the dialect proved runs
in an untracked callback keeps its proven violation even inside an uncensused
region, because that proof never came from the census. Verified live at the
current pins: `<br>{runWithOwner(owner, () => a())}</br>` — the 2.0 census gap
of the fixture below, wrapped around a callback `Solid2` reports untracked reads
in (`reports_untracked_reads_at`, `RunWithOwner` argument 1, giving
`UntrackedCallback`) — stays an SC1001 **violation**, matching its censused
control.

A hole arrives two ways, and the mitigation cannot key on which. Either the
producer never censused the expression (1.x's nested non-hydratable `<head>`;
2.0's template-root void element, which gates child lowering on
`!is_void_element`), or it censused the expression and then **retracted** the
site during lowering because the path discarded the child list (2.0's nested
dynamic-`textContent` placeholder, the textarea `value` fold, the inert
`<noscript>` fast path). Both leave the same shape, so both take the same
wording and the same verdict.

Fixtures: `fixtures/reactive-ir/jsx-census-gap-solid-1x` (a read in a nested
non-hydratable `<head>`, child and attribute arms) and
`fixtures/reactive-ir/jsx-census-gap-solid-2` (a template-root void element's
child, never censused; and a `textContent`-shadowed child, censused then
retracted). Each also pins the two negatives — a censused tracked read stays
silent, and an untracked read outside all JSX stays a proven violation.

## Divergent lowering

A census gap is the compiler saying nothing. This is the compiler saying
something true about *itself* that is false about the compiler Solid ships — and
it is the more dangerous of the two, because the fact is present, claims a
reactive rerun, and looks like any other. Before this was mitigated, every
affected shape was **silently certified**: no finding at all.

The pinned fork's `docs/execution-contract.md` names every known divergence from
the parity-target Babel plugin, with both compilers' emitted code as evidence,
and states the consumer rule as binding: **a consumer must not certify from
facts an affected divergence touches.** The trace is accurate about the fork's
output and inaccurate about the parity target, and only the consumer knows which
one it is reasoning about — which is to say, it does not know at all.

### The rules this checker applies

**A `jsx-child` execution site whose enclosing JSX element is void for the
compiler this project builds with, or is a `<noscript>`, is uncertifiable.** Not
tracked (only the fork's output tracks it), not untracked-and-stale (only Babel's
output drops it). "Void for the compiler this project builds with" is two lists —
the shared one and the dialect's parity-target-only extras; see "The void tag set
is two lists" below. Concretely:

- `<div><br>{count()}</br></div>` — the fork emits `_$insert(_el$2, count)` and
  censuses a `reactive-rerun` site; Babel discards a void element's child list in
  every position (divergence 1).
- `<noscript>{count()}</noscript>` — the fork emits `_$insert(_el$, count)` at
  template root and wherever attributes force the element off the
  static-template fast path; Babel never lowers `<noscript>` children in any
  position (divergence 3).

`divergent_lowered_child` in
`rust/crates/solid-reactive-ir/src/execution_role.rs` implements both, beside
`missing_jsx_census` because it is the same kind of thing: dialect-independent
consumer policy over producer facts, not dialect vocabulary. It carries
`ReactiveRead::divergent_lowering`, which `projection.rs` reports whatever role
the census assigned — under the pinned fork that role is `TrackedJsx`, which
reports nothing, so without this the divergence would be certified by silence —
and marks `uncertifiable`.

The carrier is an `Option<DivergentLowering>` rather than a bool because the two
conditions need **different sentences**. Babel *deletes* a void element's child
list; it never *lowers* a `<noscript>` subtree at all. "Deletes it" would name a
compiler step that does not happen, and a reader who went looking for it would
find nothing. Where the two nest, the narrowest enclosing divergent element
decides the wording: both answers are uncertifiable, only the nearer one is the
reason.

**`<noscript>` is in neither void list.** `VOID_ELEMENTS` is byte-checked against
the producers' own `void_elements` and `<noscript>` is not a member; nor is it in
either parity target's list — it has an ordinary content model and diverges
because its markup is inert. It is a separate named condition,
`INERT_MARKUP_ELEMENT`, in the same predicate. Merging them would destroy the one
property that makes the void arm auditable, and the two divergences can be fixed
upstream independently.

Three consumption points are closed. Two are pure fail-closed and one replaces a
proven claim with a weaker one; none of the three ever adds a **violation**:

- **Rerun certification.** The read becomes an uncertifiable finding where the
  census would have certified it silently.
- **Ownership attribution**, which has two halves and needs both.
  `owners.rs` skips an `ownership_regions` entry inside a divergent child, in
  both `providing_regions` and `compiler_owner_context` — the fork wraps the
  insert it emits, Babel emits neither, so the region is not evidence of an
  owner. Dropping it is only half the answer: where the surrounding context is
  *proven unowned* (module scope), the requirement then stands, and SC4001 would
  report a **proven violation** about an operation neither compiler leaves
  unowned — under the fork it runs under the insert's owner, under Babel it sits
  in deleted code and never runs. So `push_owner_requirement` — the single funnel
  both owner passes use — carries `OwnerRequirement::divergent_lowering` and
  marks such a requirement uncertain. The finding is uncertifiable and its
  message and evidence name the disagreement instead of asserting a completed
  search for an owner. Like the rerun mitigation above, this can report where
  the checker was silent before the mitigations existed; in both cases what is
  reported is a proof obligation, never a defect.
- **Reactive-reader satisfaction.** The props-destructure autofix refuses a
  rewrite whose soundness would rest on which compiler runs.

### Detection is positive, and from the AST

Two properties are load-bearing:

- **The void element comes from solid-facts' AST**, which owns JSX element-name
  syntax — never from census absence. `census_touches` overlaps deliberately, so
  a wider censused region can shadow a narrower hole; and after this pin the
  divergent child is not a hole at all, but an entry claiming `ReactiveRerun`.
- **The one compiler fact consulted is also positive**: a `jsx-expression`
  operation inside the void element's child region. That is what separates the
  divergence (the producer lowered the child) from an ordinary census gap (it
  discarded it, agreeing with Babel), and the two must not borrow each other's
  wording — one says a fact is missing, the other says two facts conflict.

### The void tag set is two lists, and one of them is dialect vocabulary

`VOID_ELEMENTS` in `execution_role.rs` is the only copy of the *producers'* tag
list in this repository, byte-checked against `void_elements` in
`packages/compiler/src/shared/constants.rs` at both pinned producer revisions
(14 tags, identical at both). A tag it missed would be a divergently lowered
child the checker certified, so the list and the compilers' must move together.

That list cannot answer the whole question, because a divergence is a producer
disagreeing with **its own parity target**, and the two parity targets disagree
with each other:

| | Rust producer | parity target |
| --- | --- | --- |
| 1.x | 14 tags (`void_elements`) | **16** — `packages/babel-plugin-jsx-dom-expressions/src/VoidElements.ts`, adding `keygen` and `menuitem` |
| 2.0 | 14 tags (`void_elements`) | 14 — `VoidElements` in `packages/runtime/src/constants.js`, imported by `babel-plugin-jsx` |

1.x's plugin computes `voidTag = VoidElements.indexOf(tagName) > -1` and gates
the whole child pass on `if (!voidTag)`, so `<keygen>{count()}</keygen>` under
1.x is the divergence exactly as `<br>` is: the producer lowers the child and
censuses `reactive-rerun`, the compiler the user builds with deletes it. Before
this was dialect-aware the checker **certified that read by silence**. 2.0's
parity target dropped both tags on purpose (its `babel-plugin-jsx/CHANGELOG.md`,
`1cc342c`, unifies the plugin's former `src/VoidElements.ts` with the runtime
set), so under 2.0 both compilers lower those children and the read is
genuinely certifiable — a merged union list would withhold a certification the
facts support.

So the extras are dialect vocabulary and travel the dialect seam:
`Dialect::parity_target_only_void_elements` in `solid-dialect`, answered
`["keygen", "menuitem"]` by `Solid1x` and `[]` by `Solid2`, each with its parity
target's file and revision named at the implementation. It is a required trait
method rather than a defaulted one: an empty answer is a claim about a specific
parity target's list, and a new dialect must make it deliberately.
`divergent_candidate_child` joins the two lists at the one question that needs
the union, so neither absorbs the other's provenance.

The dialect-only extras are not a second *kind* of divergence — they take the
same `VoidElementChild` wording, because the reason is the same one (the child
list is discarded in every position). The positive lowered-site fact still gates
them: a `<keygen>` whose children the producer discarded is an ordinary census
gap.

### Per producer

Probed, not assumed: 1.x lowers a void element's children in the *template-root*
position too, so the same source is the divergence under 1.x and an ordinary
census gap under 2.0. Fixtures
`fixtures/reactive-ir/jsx-void-child-divergence-solid-{2,1x}` hold
byte-identical sources for exactly that reason, and their snapshots now differ in
three findings:

- the template-root `<br>` child — the *producer* difference above, same verdict
  with the census-gap reason under 2.0 and the divergence reason under 1.x;
- the `<keygen>` and `<menuitem>` children — the *parity-target* difference,
  uncertifiable under 1.x and **certified (silent)** under 2.0.

The sources stay identical through both, so nothing had to be split per dialect
and `IDENTICAL_SOURCES` is unchanged. The pair also carries the ownership arms
(`CleanupInsideADivergentChild`, its certified `<span>` twin, and a plain
module-scope `onCleanup` that stays a proven violation), which answer the same
way under both dialects.

One cosmetic consequence of the `<keygen>` arms: both producers print an HTML
round-trip warning to **stderr** for it (`The HTML provided is malformed …
Browser HTML: <keygen>`), because their template validator follows the HTML
standard's legacy void list while their lowering does not. It is stderr noise
around the same disagreement the fixture pins, no gate reads it, and the analysis
result is unaffected.

The `<noscript>` positions where the fork *retracts* instead — the
static-template fast path — are not divergences: there it agrees with Babel, so
the hole is an ordinary census gap and `missing_jsx_census` answers it. That is
why the predicate is gated on a positive lowered-site fact and not on the tag,
and `RetractedInertNoscriptChild` in
`fixtures/reactive-ir/jsx-census-gap-solid-2` is the mechanical guard: keyed on
the tag, that arm would take the divergence wording and fail the gate.

The remaining divergence the fork declares — the nested `children` attribute on a
void element with no source children — is a deliberate hard reconciliation
failure in the producer, so the file is rejected rather than analyzed. It and the
1.x producer's missing retractions are recorded in `docs/precision-backlog.md`
(2026-08-24) with their status.

`Value(CallerContext)` is the dynamic component property: the expression is
handed to the child as a getter and re-evaluated in the child's tracking
context, so it is deferred rather than untracked. A component child is invoked
from the component's own render for the same reason, even though the value
itself is built once.

The hardened DOM contract covers these compiler decisions:

- Dynamic native JSX children and attributes are tracked regions.
- Expressions the compiler renders exactly once are explicit untracked
  regions: template-inlined and unwrapped-insert children (including
  `staticMarker` holes), one-shot `setAttr` attribute values, and by-value
  component properties.
- `on*` JSX values are deferred `event-handler` callbacks rather than tracked
  reads at element creation.
- Dynamic component properties and component children are deferred callbacks;
  their operations retain distinct `component-property` and `component-child`
  kinds so a component value prop is not confused with a render callback.
- `hydratable`, `dev`, `effectWrapper`, `wrapConditionals`, `staticMarker`, and
  sorted, unique `builtIns` are forwarded exactly to the compiler.
- Fact arrays are sorted deterministically by original UTF-8 byte spans.

Completeness invariant: every `jsx-expression` operation must be covered by a
tracked region, an untracked region, a callback role, or a
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

Compiler conformance — checking the Rust transform against the reference
implementation — belongs to the dom-expressions repository and runs there. To
adopt new compiler work, update the appropriate compiler `rev` in
`rust/Cargo.toml`, refresh `THIRD_PARTY_NOTICES.md`, and run `make verify`.
