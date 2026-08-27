# Resolved design decisions

These decisions are the implementation defaults. Reversing one requires new
counterevidence and, for the architectural trust decisions, a superseding ADR.

## Restricted guards

Use conjunction-only cases over selected signature, argument count,
finite literals, callable/value/Promise/AsyncIterable kind, fixed property
presence/callability, tuple alternative, result protocol, and exact artifact
case. Require a verified `otherwise` for an open-ended partition. Do not admit
general Boolean expressions or source code. Atom order is semantically inert;
non-`otherwise` cases are disjoint rather than first-match, and `otherwise` is
their verified complement.

## Consumer-driven operation vocabulary

Semantic-model version 1 carries callback invocation, return production,
reactive read/write/invalidation, resource creation, cleanup, and disposal.
Richer trace and transport details stay in sidecars until a diagnostic consumes
them.

## Cardinality scope

Every known lower or upper operation bound is scoped per trigger occurrence,
per contracted call, or per named resource lifetime. Missing bounds stay
unknown. Probes establish possibility but never a minimum, finite maximum, or
scope. Counts from different scopes never merge implicitly.

## Trigger and execution separation

Record the event or operation that triggers eligibility separately from the
event where execution occurs and whether dispatch is same-stack, queued, or
external. Timing coincidence is not causality. A generic deferred phase is not
part of normalized meaning.

## Resolver authority

Select by actual resolved artifact identity. Prefer host resolution, then Type
Facts resolution, then standalone standards-compatible package resolution.
Resolution traces are provenance, not a second matcher.

## Closure contents

Bind runtime implementation files, declarations used for proof, manifests and
resolution inputs, literal dynamic chunks, generated/virtual modules, and
relevant transform identity. External packages are accepted-contract edges.
Opaque loaders, native code, and unmaterialized transforms keep affected
domains open.

## Negative authority

The machine default is a replayable proof checked independently of generator
inference. Runtime testing alone never closes a negative. Human review is an
explicit optional fallback tier, not a normal-path gate or silent equivalent.

## Acceptance distribution

Bundled contracts compile receipts into the checker. Local contracts cache
receipts after verification. Remote contracts require local replay or a
checker-managed signed receipt. Ordinary analysis is offline and does not read
raw sidecars.

## Same-byte grouping

Group cases only when runtime hash, declaration hash, dependency closure,
transform identity, normalized export surface, and proof root match. Different
resolution traces may be retained as provenance after those identities match.

## Typestate depth

Model only finite states that change a checker conclusion: active/disposed
owner, installed/disposed cleanup, pending/settled/errored/cancelled async work,
active/settled/reverted transition, uncommitted/committed response, and
unclaimed/claimed stream. Protocol framing, codecs, and caches remain outside.

## Owner relation

Normalize owner source as none, ambient at call, ambient at execution, captured
resource, or created resource. Child-owner capability, cleanup capability,
lifetime, production, and requirements remain separate.

## Callback selectors

Type Facts supplies selected signature, actual-to-formal mapping, rest ranges,
and fixed callable paths. Reactive IR proves invocation, escape, timing,
tracking, and ownership. Variadics use universal rest selectors rather than a
finite index list.

## Experimental status

Attach experimental status to the effective export/artifact case. Entrypoint
status is shorthand only when every export shares it. Version 1 carries only
the positive experimental marker; absence means unknown, not stable. The marker
requires evidence tied to an exact published artifact or official source
revision.

## Recursive closure

Tuple items, object properties, and choice alternatives are independent local
claim domains. Closure at a composite value node applies only to its immediate
collection. Arrays separate element-shape knowledge from length knowledge.
Unknown descendants never contaminate known siblings, and parent closure never
closes a descendant.

## Frozen wire vocabulary

Use the field and enum spellings in `wire-format.md` for temporary
`schemaVersion: 2`. Summary references have no overrides; guarded behavior
lives in the summary. Wire compression may only change through an atomic
decoder/document update and cannot change `semanticModelVersion: 1` meaning.

## Digest and renumbering

Hash normalized meaning under `semanticModelVersion`, independent of wire
schema version and compression. The temporary `2 -> 1` cut may reuse unchanged
fact/probe evidence but always re-emits wire bytes and receipts.

## Compactness budget

Size is a performance target, never a reason to omit proof semantics. Target
p95 32 KiB per artifact case and 128 KiB per package, with a 1 MiB safety cap,
bounded depth, and linear normalization.

## Automatic-generation boundary

Leave affected domains open for nonliteral loading, runtime code generation,
opaque native/WASM code, Proxy/reflection, unknown callback escape, open-world
dispatch, or arbitrary race/network-dependent behavior. Continue accepting
independent domains.

## TypeScript overlap

Gate diagnostics, not merely contract facts, against exact published typings.
Invalid callback return types, impossible overloads, missing members, and
non-callable values are common duplication risks. Ownership, tracking, timing,
undisposed cleanup, response commitment, and artifact-specific runtime behavior
remain checker claims when TypeScript is silent.

## Fact-producer investment

Improve the local Type Facts module and compiler execution facts before
expanding the public schema with workarounds. Type Facts owns language
semantics; compiler facts own actual lowering; Reactive IR composes them;
contracts own external runtime behavior.

## Type Facts source ownership

Bring the Type Facts Go producer and Rust client back into this repository so a
new fact lands atomically with its checker consumer and corpus evidence. Retain
the versioned process/session interface; colocation does not allow Reactive IR
to depend on TypeScript-Go or transport internals. Import and prove parity
before adding new demands, then retire the external repository as an active
pull-request target.

## Compiler fork scope

The Solid 2 fork contains semantic-fact code only: trace models, output-neutral
recording hooks, validation/serialization, and fact-specific tests. It carries
no lowering, output, diagnostic, runtime, feature, performance, or unrelated
compiler change. Discovered compiler defects go upstream independently; until
the upstream base contains the fix, the corresponding fact stays open.
