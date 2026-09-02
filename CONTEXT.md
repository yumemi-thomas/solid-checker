# Solid Checker

Solid Checker certifies the reactivity and asynchronous behavior of Solid
TypeScript projects without coupling its analysis to one compiler backend.

## Language

**Type Facts**:
Compiler-independent semantic facts about a configured TypeScript project.
_Avoid_: Compiler facts, checker data

**Async function fact**:
A semantic summary of a function-like declaration or expression, including whether it can return asynchronously and which calls are dominated by an await.
_Avoid_: Async scan result, async metadata

**Reference index**:
The generation-scoped mapping from durable symbol identities to their source reference locations.
_Avoid_: Reference cache, usage map

**Type Facts session**:
A retained analysis lifetime for one configured TypeScript project, carrying its current generation and acknowledged demand state across requests.
_Avoid_: Lifecycle responder, retained protocol state

**Type Facts producer**:
The repository-owned process that answers Type Facts demands for one configured TypeScript project. It stays behind the versioned process/session protocol.
_Avoid_: Server, backend, external sidecar

**Generation**:
A numbered Type Facts project state. Every accepted update advances it exactly once; generation-scoped identities must be reacquired afterward.
_Avoid_: Revision, snapshot

**Semantic demand run**:
One source file's canonically ordered Type Facts demands for a generation.
_Avoid_: Query batch, demand group

**Wire table transition**:
A deterministic transport frame that establishes a Type Facts table or transforms the table named by its base generation and state token.
_Avoid_: Payload, packed delta

**Configured source descriptor**:
A Type Facts session source entry that names a canonical disk path for local hydration, while edited or virtual sources remain inline. Generation source hashes still prove that Rust and TypeScript-Go analyzed identical content.
_Avoid_: Path-only source, source shortcut

**Semantic lookup**:
The project-wide query surface rule discovery asks for semantic answers — the entity or symbol at or containing a location, the function a symbol names, whether an owner is rendered under a Loading boundary — instead of scanning fact tables.
_Avoid_: Index helpers, fact-table scan, range-query module

**Fact domain**:
One of the independent evidence suppliers the checker cross-references: Oxc syntax facts, Solid compiler execution facts, Type Facts, and package contracts. User-facing documentation may call them "sources of evidence"; the canonical term is fact domain.
_Avoid_: Backends, sidecars, analysis inputs

**Compiler semantic trace**:
The compiler-owned, transform-local record of semantic decisions made while lowering one source file. It observes existing lowering and is versioned separately from the normalized compiler execution facts consumed by the checker.
_Avoid_: Execution map, compiler sidecar, instrumented output

**Compiler execution facts**:
The checker-normalized fact domain projected from a validated compiler semantic trace, binding source sites to actual execution, callback, ownership, and discarded-code decisions. Its protocol version is independent of the producer trace version.
_Avoid_: Type Facts, compiler predictions, semantic trace (that is the producer form)

**Claim domain**:
One independently knowable set of package behavior, such as callback invocations, reactive reads, or cleanup operations. Its knowledge may be open or closed without determining any sibling claim domain.
_Avoid_: Fact domain, section, field group

**Domain closure**:
Accepted proof that one claim domain enumerates every behavior possible for its exact package, export, artifact case, guard, and resource scope. Closure never transfers implicitly to a parent, child, or sibling claim domain.
_Avoid_: Complete contract, tested absence, empty result

**Claim knowledge**:
The local knowledge state of one claim domain: unknown, partial positive, complete positive, or complete negative. A state says nothing about a parent, child, sibling, alternative, or referenced resource unless that subject has its own claim knowledge.
_Avoid_: Status wrapper, inherited completeness, contract completeness

**Operation cardinality**:
A proved lower and upper bound on how often one semantic operation occurs within an explicit scope such as one trigger occurrence, one package call, or one resource lifetime. Possibility, guarantee, repetition, and bound scope are separate facts.
_Avoid_: Call count, observed frequency, phase repetition

**Guard partition**:
A finite set of statically decidable contract alternatives selected from exact call, value, protocol, or artifact facts. A complete partition is proved disjoint and exhaustive; unresolved selection joins every possible alternative.
_Avoid_: Runtime condition, source expression, heuristic branch

**Contract proposal**:
An unaccepted machine description of package behavior together with proof obligations. A proposal cannot certify a project or discharge a proof obligation.
_Avoid_: Generated contract, inferred contract

**Accepted package contract**:
A normalized package contract whose closed claims, package identity, artifact cases, and proof inputs are bound by a valid acceptance receipt. Open claims remain usable only as partial knowledge.
_Avoid_: Verified JSON, trusted contract, reviewed contract

**Normalized contract model**:
The rich, wire-independent semantic representation produced by the single contract decoder and normalizer. Reactive IR consumes this model and never compact-document omission, summary, closure-array, or schema-version conventions.
_Avoid_: Expanded JSON, decoded schema, contract AST

**Artifact case**:
One exact package-entrypoint resolution outcome, including its resolution trace, runtime artifact, declarations, and dependency closure. Artifact cases are selected exclusively and never merged.
_Avoid_: Environment, mode, variant

**Operation graph**:
A causal description of package behavior whose nodes are semantic operations and whose edges express scheduling, data, cleanup, error, or lifetime relationships.
_Avoid_: Phase list, callback list, execution trace

**Evidence sidecar**:
A hash-bound artifact containing detailed fact transcripts, probe observations, and proof material for package-contract claims. Ordinary project analysis does not need it after acceptance.
_Avoid_: Contract evidence field, audit log

**Proof demand**:
A verifier-derived, policy- and artifact-snapshot-bound requirement for one exact claim or positive fact, assigned to its authoritative fact owner. Callers may transport its opaque identity but cannot create, omit, satisfy, or declare it inapplicable.
_Avoid_: Proof checklist, caller obligation, requested evidence field

**Artifact snapshot**:
The immutable content-addressed view of one independently acquired package artifact used for every read during a certification transaction. It is distinct from an artifact case (a selected resolution outcome) and from a Type Facts generation (a project-analysis state).
_Avoid_: Package directory, temporary extraction, artifact case, generation

**Probe gate**:
A verifier-derived mandatory contradiction veto for one proposed closed claim. A contradiction blocks closure; success or finite non-observation never proves absence, completeness, or safety.
_Avoid_: Runtime proof, passing probe, negative observation

**Verified positive fact**:
An analyzer-visible possible behavior retained only after its exact proof demand has an authoritative witness. It does not close its surrounding claim domain or imply that unobserved sibling behavior is absent.
_Avoid_: Partial proof, inferred behavior, closed claim

**Acceptance receipt**:
A verifier-issued binding among a contract's wire and semantic identities, exact artifacts, proof material, and verification policy. It is the authority ordinary analysis uses to accept closed claims.
_Avoid_: Verification report, signature, trust flag

**Issuer provenance**:
The authenticated channel through which a receipt gains trust: compiled
built-in bytes, a configured persistent-local issuer, or an explicitly trusted
portable issuer chain. A key ID, public key, or signature carried only inside
the receipt is never issuer provenance.
_Avoid_: Self-signed receipt, receipt key, trusted signature

**Semantic digest**:
A canonical identity of normalized package-contract meaning that excludes wire version, summary names, formatting, and evidence layout.
_Avoid_: File hash, contract hash

**Rule options**:
The project-level per-rule configuration document, `.solid-checker/rule-options.json`, discovered beside a project's contracts and carrying the upstream eslint-plugin-solid options the 1.x rules honour. Defaults are upstream's defaults; parsing fails closed. Part of every build and diagnostic identity.
_Avoid_: Rule config, checker settings, options file

**Finding kind**:
Whether a finding is a **violation** (the analyzer proved the code misbehaves at runtime) or **uncertifiable** (a proof obligation the analyzer could not resolve). Distinct from severity (error/warning).
_Avoid_: Finding status, finding type

**Discarded region**:
A source region the Solid compiler censused and then **deleted** — the `Value(Elided)` decision, projected as `ExecutionMap::discarded_regions` and classified `ExecutionRole::DiscardedRendering`. Distinct from an *untracked region*, which is code that executes once at render: a discarded region executes zero times, so it supports no finding and no certification. Silence over one means "both compilers deleted this", never "this was proven safe".
_Avoid_: Elided region, dead region, untracked region (that is the once-executing one), unreachable code

**Failure class**:
A user-facing grouping of the runtime misbehavior that findings prevent: silent staleness (reads that register no dependency), feedback loops (writes and actions in owned scopes), escaped async (pending reads outside tracked or Loading-bounded regions), and lifecycle leaks (effects, cleanups, and boundaries without a live owner).
_Avoid_: Bug category, rule group (that is the SCxxxx numbering)
