# Phase 5 completion report: normalized semantic model

Date: 2026-08-27

Branch: `codex/phase5-normalized-semantic-model`

Authority: published `solid-js@2.0.0-rc.3`, related published `@solidjs/*`
artifacts, the frozen semantic-model documents, and the Phase 4 compiler-facts
boundary

## Result

Phase 5 items 55-67 are implemented in
`solid-reactive-ir::contract_semantics`. The module now accepts an untrusted
semantic proposal, validates and canonicalizes it, and returns a private-field
`NormalizedContract` with a canonical semantic digest. Compact wire mechanics
do not occur in the model.

This phase does not accept proposed closure. `AcceptedContract` has no public
constructor; the future proof replay and receipt authority must authorize every
closed claim. The backend wire decoder continues to return
`NormalizationUnavailable`, and no generator, evidence sidecar, receipt,
analyzer consumer, bundled contract, or public schema was migrated.

## Normalized knowledge

`KnowledgeSet<T>` represents the four local states without an ambiguous open
empty encoding:

| State | Internal representation | Meaning |
| --- | --- | --- |
| unknown | `Unknown` | no positive or negative conclusion |
| partial positive | non-empty `Partial(items)` | every item is known; more may exist |
| complete positive | non-empty `Complete(items)` | the items exhaust this immediate domain |
| complete negative | empty `Complete(items)` | this immediate domain is proved absent |

An empty `Partial` is rejected. Joining unresolved alternatives unions every
positive item and retains closure only when every possible alternative is
closed. An unknown alternative therefore retracts negative proof but never
erases a known positive sibling.

Recursive shapes carry this state at each tuple-item, object-property, and
choice-alternative collection. Arrays carry one independently traversed element
and independent minimum/maximum length premises. Promise and AsyncIterable
values recurse through distinct path segments. `ClaimPath` identifies the
value root, exact recursive path, and exact open domain, so an unknown nested
leaf does not open its parent or a sibling.

## Operations, causality, resources, and lifetimes

The model separates these operation axes:

- kind and positive call-domain claim;
- guard;
- trigger cause;
- execution point;
- schedule;
- tracking;
- owner source, requirements, capabilities, production, and lifetime;
- cardinality scope, minimum, and maximum;
- ordered inputs, optional output, and referenced resources.

Operation edges use the finite `orders`, `data`, `invalidates`, `error`,
`cleanup`, and `lifetime` relations. References must resolve and the complete
causal graph must be acyclic. Repetition is represented by trigger and
cardinality rather than cycles. Cardinality distinguishes possible behavior
(`min` absent or zero) from guaranteed behavior (`min >= 1`), preserves finite
versus unbounded maxima, and requires one explicit call, trigger, or named
resource scope whenever a bound is present.

Resources provide exact cross-operation anchors for owners, reactive sources,
async computations, transitions, cleanups, requests, responses, streams, and
server-function references. Resource state and capability vocabularies are
kind-checked. Lifetimes name their exact resource and are kind-checked for
owner, request, transition, and async-source relations.

Ownership requirements, actual source, available child/cleanup capabilities,
produced owners, and lifetime are independent values. A created owner must
have a separate positive production claim; creation is never inferred from a
requirement or capability. Contradictory required/forbidden combinations and
resource-bound lifetimes without an owner source are refused.

## Values, capabilities, guards, and identities

The recursive value model covers unknown/plain/parameter values, tuples,
arrays, objects, choices, callables, Promise, AsyncIterable, reactive accessors
and setters, stores, actions, components, cleanups, ref applications, and
server-function references. Projection and snapshot behavior is represented
by locally closed capability sets and resource relations rather than a nominal
runtime category.

Validation rejects, among other contradictions:

- readable setters and writable accessors;
- a closed accessor/setter/store capability set that omits its required
  readable or writable capability;
- optimistic state without both an intrinsic writable claim and a writable
  transition resource;
- refreshability without a positive refreshable resource claim;
- pending-aware values not bound to an async resource;
- cleanup callables with neither a cleanup resource nor a lifetime;
- incompatible resource states/capabilities and dangling resource references.

Guards are conjunctions of selected signature, argument-count, finite literal,
value-kind, fixed-property, tuple-alternative, result-protocol, and exact
artifact-case atoms. Atom order and finite-number spelling are canonicalized.
Duplicate or unsatisfiable atoms, overlapping cases, multiple `otherwise`
cases, a closed non-empty partition without `otherwise`, and an open partition
with `otherwise` are refused. Unknown selection joins every possible case
monotonically.

Artifact cases bind the package version/integrity and manifest digest to the
requested entrypoint, exact resolution trace, runtime/declaration artifacts,
dependency closure, optional transform, and export surface. Cases with an
identical selection identity are refused instead of merged. Each export binds
its public name and exact runtime/declaration export target to its artifact
case. Experimental evidence is local to an effective export or case; absence
continues to mean unknown, never stable.

## Canonical semantic digest

The digest is SHA-256 over typed, length-delimited values under the domain
`solid-checker:normalized-package-contract` and
`SEMANTIC_MODEL_VERSION == 1`. Normalization sorts every semantically unordered
collection, preserves tuple/input/resolution-trace order, puts `otherwise`
last, lowercases artifact digest hex, and canonicalizes finite number guards.
The four knowledge states have distinct tags.

The digest includes exact package manifest, artifact, declaration, transform,
closure, export, operation, resource, guard, stability, and behavior meaning.
It excludes JSON formatting/key order and does not contain wire schema version,
summary references, aliases, `closed` arrays, omission conventions, evidence
paths, or receipt bytes. Operation/resource IDs are already semantic local IDs
at this boundary; Phase 6 summary expansion must alpha-rename wire-local IDs
before constructing a proposal.

## Solid 2 conformance matrix

Every row has a normalized representation. “Open” below names only the exact
premises Phase 5 does not establish; known siblings remain usable.

| Matrix row | Normalized representation | Exact domain still open or refused |
| --- | --- | --- |
| Split `createEffect` | compute/apply/cleanup operations, queue and flush events, data/error/cleanup edges, defer guards, tracking and cardinality | exact RC.3 callback/runtime proof, wrapper-emission census, cleanup replacement/disposal observation |
| `createTrackedEffect` / `onSettled` | callback operations, captured/created owners, forbidden child/cleanup capabilities, cleanup return and owner lifetime | package-runtime owner and returned-cleanup proof |
| Batched writes / `flush()` | write/invalidate/apply operations, queued schedule, flush point and causal barrier edges | controlled runtime settled-value and drain trace |
| `For` / `Repeat` / `Show` / `Match` | finite guards plus tuple/choice/reactive callback input shapes | exact Type Facts signature partition and runtime callback arguments; dynamic options remain an unresolved join |
| Promise / AsyncIterable computations | Promise/AsyncIterable leaves, async resource states, settle/emission/error/disposal operations | repeated emission, rejection, cancellation, and disposal runtime proof |
| Loading / pending / latest / refresh / affects | readable, pending-aware, refreshable capabilities bound to exact async/reactive resources | branded declaration identity and runtime state-transition proof |
| Actions / optimistic state | action and store shapes, transition resource, writable/optimistic capabilities, settle/error/revert states | async-generator/action identity and optimistic lifecycle proof |
| Store drafts / projections / snapshots | store shapes and locally closed readable/writable capabilities; complete-readable without writable expresses snapshot immutability | opaque helper behavior, deep/shallow tracking, reconciliation, and dependency contracts |
| Two-phase refs / directives | factory/apply operations, ref-application shape, render/cleanup timing and edges | Phase 4 generated-operation census, `@solidjs/web` runtime callback/cleanup contract |
| Root-owned event delegation | external callback operations bound to owner resources and owner lifetime/disposal | renderer registration, multiple-root, and disposal runtime proof |
| Browser render/hydrate / SSR | exclusive exact artifact cases, owner/callback operations, response/stream resources and lifetimes | Phase 7 artifact selection plus DOM/hydration/stream runtime proof; universal/dynamic compiler modes remain refused |
| `httpStatus` / `httpHeader` | request/response resources, uncommitted/committed states, request lifetime and mutation operations | request-scope runtime and post-commit refusal proof |
| Server-function references | exact transform/artifact/export identity, reference resource/shape, Promise result and operations | Phase 4 registration facts are positive; transport, serialization, runtime invocation, and receipt composition remain open |
| Experimental server components | case/export-local experimental marker plus leaf-local open protocol shapes | unstable protocol leaves remain open until exact RC.3 evidence proves each one |
| Conditional adapters | finite guard partitions, branch-specific value choices, shared subgraphs and monotone unresolved selection | non-finite selection and unobserved branch behavior remain open |
| Mixed-framework packages | exact entrypoint trace, artifacts, dependency closure and export targets | Phase 7 closure/resolution proof; ambiguous or mixed artifacts must be refused |

The focused conformance test constructs and normalizes one representative for
all sixteen rows. It is a model-expressiveness test, not acceptance evidence.

## Tests

Twenty-one focused tests cover:

- all four local knowledge states and false open-empty closure;
- monotone joins over generated alternative subsets;
- independently open operation axes and recursive leaf locality under sibling
  permutations;
- missing graph nodes, causal cycles, invalid cardinality, contradictory owner
  relationships, missing owner production, incompatible resource states, and
  capability contradictions;
- unsatisfiable/overlapping guards, exhaustiveness rules, complete-negative
  guard selection, and unresolved joins;
- exact export identity, duplicate artifact selection, and local experimental
  status;
- order-equivalent normalization, repeated deterministic digests, canonical
  numeric guards, and distinct digest tags for all knowledge states;
- normalized representations for all sixteen conformance rows.

## Facts and generated artifacts

No Type Facts producer/client/schema, Solid compiler fork, compiler identity,
compiler-facts protocol, generated contract, bundled contract, snapshot, or
other generated artifact changed. Phase 5 exposed no premise that required a
new local Type Fact or compiler semantic fact.

## Verification

Focused checks completed:

- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib contract_semantics::tests` — 21 passed.
- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib` — 168 passed.
- `cargo +1.97 clippy --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib --tests -- -D warnings` — passed.
- `cargo +1.97 fmt --manifest-path rust/Cargo.toml --all` and `git diff --check` — passed.

Full handoff authority:

- `make verify` — passed in 74.48 seconds. This included formatting, Go vet and
  race tests, workspace-wide Clippy/tests/doc-tests, both single-dialect backend
  and WASM builds, compiler identity, Type Facts stamp verification, coverage
  (94 fixture projects / 557 findings), ownership (289 cases / 465 ledger
  rows), performance certification, CLI tests (46), the Phase 0 baseline,
  TypeScript oracle tests/gate, obligation audit, miscellaneous lint, and
  bundled-contract conformance/pin checks.

Commit identities and the pull-request URL are reported in the pull request
and final handoff rather than embedded here, which avoids a self-referential
documentation commit.
