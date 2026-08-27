# Phase 2 deep module interface freeze

Date: 2026-08-27

## Outcome

Phase 2 is complete. The replacement contract path now has a wire-independent
semantic model in Reactive IR and one backend loading boundary. Current
schema-v1 contracts and their consumers remain untouched.

The development-schema loader parses and checks the private envelope, enforces
the 1 MiB document limit, checks receipt version and exact wire digest, and
then returns `NormalizationUnavailable`. This is an intentional structural
refusal, not semantic uncertainty: accepting schema-v2 input before canonical
normalization and full receipt binding exist would create an unverified trust
path.

## Interface ownership

| Owner | Interface | Contract |
| --- | --- | --- |
| `solid-reactive-ir::contract_semantics` | `KnowledgeSet<T>` | Represents unknown, partial positive, complete positive, and complete negative knowledge without an open-empty state |
| `solid-reactive-ir::contract_semantics` | `ContractProposal`, `EvidenceBundle`, `AcceptanceReceipt`, `AcceptedContract` | Keeps proposal, evidence, receipt, and accepted typestates distinct |
| `solid-reactive-ir::contract_semantics` | operations, resources, guards, values, ownership, cardinality, artifact cases | Uses the Phase 1 normalized vocabulary and contains no compact-wire fields |
| `solid-reactive-ir::contract_semantics` | `AcceptedContract::export`, `ExportSemantics::claim`, `operation`, `unresolved_claims` | Lets consumers query semantic outcomes without schema knowledge |
| `solid-facts-backend` | `load_accepted_contract` | Is the only public document-plus-receipt loading function |
| `solid-facts-backend` | `ArtifactResolver` | Produces one exact `ResolvedImport` or a typed unattested/ambiguous/invalid refusal |
| `solid-facts-backend` | `HostResolutionAdapter`, `StandaloneResolutionAdapter` | Preserve the authority of complete Type Facts/host and standalone package-resolution records; neither guesses resolution |
| `solid-facts-backend` | `EvidenceStore` | Reads accepted receipts without exposing raw proof sidecars to ordinary analysis |
| `solid-facts-backend` | `BundledEvidenceStore`, `LocalEvidenceStore` | Supply bundled and project-local content-addressed receipt storage and rehash bytes on every read |

## Exact-resolution record

`ResolvedImport` carries all selection inputs that a later normalizer must
compare rather than infer:

- specifier, importing file, and requested entrypoint;
- package name, version, integrity, and manifest;
- runtime artifact path and digest;
- declaration path and digest;
- dependency-closure digest;
- optional transform path and digest;
- ordered package-export branch trace;
- resolution authority.

The adapters retain duplicate exact rows and refuse them as ambiguous. They do
not collapse duplicates with last-writer-wins maps. A standalone resolver lives
in package acquisition; the Rust adapter consumes its exact result and does not
reimplement conditional exports.

## Failure locality

`ContractFailure` separates document size/decode, receipt decode/version,
unsupported schema, unavailable normalization, zero/multiple artifact cases,
identity mismatch, receipt mismatch, and invalid normalized semantics.
`ArtifactResolutionFailure` separately represents unattested, ambiguous, and
structurally invalid resolution. `EvidenceStoreFailure` distinguishes invalid
keys, I/O, and content mismatch.

None of these failures is converted into negative semantic proof. Open semantic
claims remain `KnowledgeSet::Unknown` or `Partial`; structural failures refuse
the selected artifact case.

## Interface tests

The Phase 2 tests establish that:

- malformed documents fail through the single public loader;
- valid temporary schema-v2 envelopes remain refused until normalization;
- stale wire digests fail before normalization;
- duplicate exact resolution results are ambiguous;
- the two resolver adapters preserve their authority;
- bundled and local receipt stores are content addressed;
- tampered receipt bytes are rejected;
- unknown and complete-negative claims remain distinct;
- semantic queries expose claim knowledge and unresolved obligations without
  exposing summary IDs or `closed` arrays.

## Deliberately deferred

Phase 2 defines and tests the boundary; it does not claim the replacement
format is loadable yet. These operations remain assigned to later phases:

- invariant validation, monotone joins, capability contradiction checks, graph
  validation, and recursive unresolved-claim traversal: Phase 5;
- private wire expansion, alpha-renaming, exact artifact-case selection,
  canonical hashing, and full receipt validation: Phase 7;
- proof replay and receipt issuance: Phase 9;
- movement of current analysis consumers: Phase 10;
- generator migration: Phase 11;
- temporary-version renumbering and stable cutover: Phase 14.

Until those phases land, no code path can construct an accepted contract from
schema-v2 bytes. That refusal is the accuracy-preserving behavior.

## Deletion test

The new Reactive IR module has no serde schema annotations and names no summary,
closure-array, omission, alias, or schema-version concept. The private wire
types are not exported by the backend. A future caller can depend on accepted
artifact identity, export semantics, claim knowledge, operation lookup, and
unresolved obligations using only the normalized module. Therefore compact
wire knowledge terminates at the backend boundary as required by the Phase 1
deletion test.
