# Phase 10 completion report: authoritative runtime probes

Date: 2026-08-27

Branch: `codex/phase10-authoritative-runtime-probes`

Authority: semantic-model version 1, the Phase 7 exact artifact/closure seam,
the Phase 8 unaccepted proposal plan, the Phase 9 claim/evidence identities,
published Solid RC.3 artifacts where runtime recipes will later be attached,
and the rule that finite execution cannot prove absence

## Result

Phase 10 items 108-116 are implemented. The backend now owns a deep semantic
runtime-probe module from exact plan construction through deterministic
transcript evaluation. Node remains the eventual owner of package acquisition,
worker launch, and runtime interaction, but it cannot classify worker output or
promote a semantic claim.

The evaluator can produce only:

- a possible-positive witness for one exact planned possible operation;
- a contradiction that falsifies one exact planned closure candidate;
- an error, bounded timeout, or local refusal.

It cannot produce complete-negative knowledge, guaranteed behavior, a positive
minimum, a finite maximum, exhaustiveness, accepted closure, a proof root, a
receipt, or an `AcceptedContract`. Phase 11 remains the sole acceptance
authority.

## Exact artifact-mode planning

`ArtifactModeMatrix` binds every mode to one normalized artifact case and one
canonical `EnvironmentIdentity`. Environment identity includes runtime name,
version, build and protocol, OS, architecture, sorted conditions, and explicit
sandbox kind/policy. Unknown artifact cases, duplicate case/mode pairs, empty
matrices, invalid sandbox claims, and unbounded inputs are rejected.

`RuntimeProbePlan::for_proposal` accepts recipes only from the exact Phase 8
`PlannedProposal`:

- `PossiblePositiveWitness` must name a planned possible operation subject,
  and its expected event must name that same operation;
- `ClosureFalsification` must name a planned closure-domain subject;
- every targeted artifact case must have at least one exact mode;
- one semantic subject cannot receive competing recipes.

Plan, recipe, and session identities use typed, length-delimited SHA-256
domains. Plan identity binds the normalized semantic digest, policy, complete
mode/environment matrix, claim IDs, and canonical recipe digests. Session
identity additionally binds exact artifact case, mode, and repeat ordinal.
Input ordering, environment-condition ordering, and coverage-limitation
ordering cannot change the result; changing a real mode or environment
identity does.

## Semantic events instead of timing guesses

Recipes contain semantic drain steps rather than sleeps:

- runtime flush;
- a bounded number of microtask turns;
- a bounded number of macrotask turns.

Wall-clock time is only a hard termination bound. It never determines whether
a callback was inline, scheduled, absent, or exhaustive. Both global policy
and each recipe's actual drain budget are checked.

The event vocabulary records:

- call enter/exit;
- render enter/exit;
- flush ordinal;
- callback ordinal;
- cleanup registration, production, invocation, and disposal;
- settlement, rejection, and cancellation;
- indexed async emission;
- transition active, settled, and reverted states;
- request enter/exit;
- response uncommitted/committed state; and
- stream open, chunk, close, and cancellation state.

Events carry a contiguous zero-based semantic sequence, a stable marker, an
optional exact normalized operation, and resource identity where applicable.
Malformed or unbounded event sequences are rejected rather than interpreted.

## Isolation, bounds, and repeat consistency

Every artifact-mode target expands to at least two and at most sixteen repeat
sessions. Each returned run must name a fresh process, realm, and module
instance. Reusing any of those identities across any supplied session refuses
the affected observations, preventing mutable module state or shared realms
from manufacturing consistency.

Runs must match the planned canonical environment, remain within the recipe's
microtask/macrotask budget, and stay within the event and timeout bounds. A
usable mode requires every repeat and byte-for-byte equivalent semantic event
vectors across repeats. Missing repeats, worker refusals, timeouts, errors,
environment mismatch, excess drains, or nondeterministic events remain scoped
to that exact claim/mode. A valid sibling mode is retained.

## Lifecycle scenarios

Six typed scenarios constrain what a positive marker is allowed to mean:

- `Operation` requires the exact planned positive marker;
- `CleanupLifecycle` requires cleanup registration/production before later
  invocation/disposal;
- `RepeatedAsyncIterable` requires at least two zero-based consecutive
  emissions for one resource followed by settlement/rejection/cancellation;
- `TransitionLifecycle` requires active followed by settled or reverted for
  one resource;
- `RequestResponseLifecycle` requires request entry followed by uncommitted
  and then committed response state for one resource;
- `RootLifetime` requires invocation/disposal explicitly marked as bound to
  the root lifetime.

The transcript may record other positive events, but they do not satisfy a
recipe unless its exact marker, event class, operation binding, and scenario
lifecycle all match.

## Witness and falsification authority

A completed finite run with the expected marker can witness that one possible
operation occurred. The same positive-event mechanism can contradict a
proposed closed domain when the recipe is authorized against an exact closure
candidate. It does not directly edit the normalized model.

The absence of a marker is always a refusal. So are an incomplete lifecycle,
inconsistent repeats, timeout, error, or malformed environment. None becomes
negative evidence. Closure falsifications are emitted as typed
`ProbeContradictionRecord` values for Phase 11 replay; they are not acceptance
decisions.

## Canonical transcripts and evidence integration

Successful witness/falsification modes emit deterministic internal transcript
documents with:

- document kind and transcript version;
- semantic-model version and semantic digest;
- probe-plan digest and semantic claim ID;
- exact artifact case, entrypoint, runtime/declaration artifacts, dependency
  closure, and optional transform;
- export, authority, scenario, recipe, mode, and canonical environment;
- every isolated repeat/session identity and semantic drain count; and
- the complete ordered semantic events.

Pretty JSON plus one trailing newline is SHA-256 addressed. Reordering worker
responses does not change bytes or digest. This internal transcript is not the
temporary public main wire format.

The Phase 9 probe sidecar now stores a bounded, non-empty `observations` matrix
per semantic claim. Each mode independently records environment and outcome.
Emission sorts modes; validation rejects empty, duplicate, unsorted, invalid,
or excessive observations. `EvidenceCatalog::for_proposal` admits both planned
possible-operation witnesses and planned closure falsifiers.

## Focused and adversarial tests

Backend tests cover:

- exact artifact-mode expansion and bounded repeat sessions;
- all eleven event families and transcript discriminators;
- canonical plan, session, transcript, environment, and input-order identity;
- fresh process, realm, and module state;
- missing repeats, finite absence, timeout, error, excess drain, wrong
  environment, malformed sequence, and nondeterministic-repeat refusal;
- valid sibling-mode preservation after local inconsistency;
- positive-only closure falsification and absence that cannot confirm closure;
- ordered cleanup, repeated AsyncIterable, transition, request/response, and
  root-lifetime lifecycles;
- refusal of incomplete cleanup lifecycles;
- plan-time rejection of unauthorized promotion;
- the public Phase 8 proposal-to-Phase 10 plan boundary;
- multi-mode Phase 9 sidecar integration; and
- empty, duplicate, unsorted, and noncanonical sidecar mode matrices.

## Type Facts, compiler facts, and generated artifacts

No Type Facts producer, Rust client, schema/protocol, build identity, fixture,
or normalized consumer changed. No Solid compiler fork, compiler semantic
trace, compiler-facts protocol, compiler pin, identity document, notice, or
conformance report changed.

The checked-in evidence-sidecar JSON Schema changed from one environment/
outcome pair per claim to the required claim-local exact mode matrix. This is
a source schema refinement before the public Phase 14 cutover. No transcript
instance, temporary main contract, bundled contract, package-contract fixture,
acceptance receipt, analyzer snapshot, dialect manifest, runtime lock, binary,
or other generated artifact changed.

## Verification

Focused iteration:

- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib runtime_probes`
  — 12 passed;
- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib evidence_sidecars`
  — 8 passed;
- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib proposal_generation`
  — 8 passed;
- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib`
  — 74 passed;
- focused backend Clippy with `-D warnings` — passed;
- `jq empty schema/solid-reactivity-evidence-sidecars-v1.schema.json` — passed;
- `git diff --check` — passed.

Final handoff authority:

- `make verify` — passed in 132.97 seconds;
- workspace unit, process, integration, and documentation tests — passed,
  including all 74 backend library tests;
- coverage — 94 fixture projects and 557 findings matched;
- ownership gate — 289 cases passed and all 465 ledger rows are resolved;
- TypeScript oracle — 161 cases held on both the TypeScript and checker sides,
  with 41 reporting-rule keystones;
- obligation audit — 7 obligations held and 11 closures discharged theirs;
- performance certification — passed;
- CLI — 7 test files and 60 tests passed;
- contract/TypeScript probe suites — 26 test files and 333 tests passed; and
- compiler identity, dialect manifests, schema/lint gates, package pins, and
  bundled-contract conformance — passed.

## Exact remaining open or uncertifiable cases

- Phase 11 must replay proof families and `ProbeContradictionRecord` values,
  finalize only verified local closure, compute proof/closed-claim roots, issue
  receipts, and construct accepted contracts.
- Phase 13 must attach exact published RC.3 recipes and conformance fixtures to
  every runtime-dependent matrix row. The semantic scenario vocabulary exists;
  this phase does not claim that every RC.3 row has already executed.
- Phase 14 must switch the public JavaScript probe plan, worker, driver, and
  harness to the Rust-owned session/transcript interface. The legacy timing-
  based public probes remain unchanged until that atomic migration.
- Public generators, verifiers, bundled contracts, backend process fixtures,
  WASM adapters, and analyzer consumers remain on their scheduled migration
  phases. No current finding is reduced by this abstraction alone.
- A missing expected marker, finite non-occurrence, timeout, error, refusal,
  inconsistent repeat, or unavailable environment remains uncertifiable for
  that exact claim/mode. It is never negative proof.
- Dynamic loading, native addons, opaque WASM, mutable globals, unmaterialized
  transforms, unaccepted dependency edges, incomplete compiler emission
  censuses, universal/dynamic compiler modes, and experimental server-
  component behavior remain open exactly where earlier phases recorded them.

No upstream Solid pull request is required or authorized for this phase.

## Handoff

- Branch: `codex/phase10-authoritative-runtime-probes`
- Implementation commit: `88752e6a` (`feat(contracts): add authoritative
  runtime probe model`)
- Architecture/report commit: `d3857662` (`docs(contracts): record phase 10
  completion`)
- Pull request:
  [yumemi-thomas/solid-checker#52](https://github.com/yumemi-thomas/solid-checker/pull/52)

No upstream Solid pull request was created.
