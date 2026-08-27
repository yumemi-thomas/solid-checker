# Phase 6 completion report: temporary wire schema v2

Date: 2026-08-27

Branch: `codex/phase6-temporary-wire-schema-v2`

Authority: the frozen package-contract-v2 model, published
`solid-js@2.0.0-rc.3` and related `@solidjs/*` artifacts where behavioral
examples are relevant, and the Phase 5 wire-independent semantic model

## Result

Phase 6 items 68-80 are implemented. The crate-private
`solid-facts-backend::contract_document_v2` module owns the complete temporary
`schemaVersion: 2` document, expands its compact mechanics, and submits only a
`ContractProposal` to `solid-reactive-ir::contract_semantics`.

The external `load_accepted_contract` interface now performs strict document
decoding, receipt-envelope checks, summary expansion, and semantic
normalization. It then returns `AcceptanceUnavailable`. This is deliberate:
Phase 6 can validate an untrusted proposal, but only Phase 11 proof replay and
receipt verification may construct `AcceptedContract` and authorize closure.

The existing legacy schema-v1 adapter, current generator, bundled contracts,
and analyzer consumers are unchanged. The new schema is a temporary
development artifact and is not the later stable public schema-v1 cut.

## Wire envelope and exact identity

The decoder requires:

- `format: "solid-reactivity-contract"`;
- temporary `schemaVersion: 2` and `semanticModelVersion: 1`;
- exact package name, version, published integrity, and manifest path/digest;
- unconditional artifacts or explicit conditional cases, never a mixed form;
- runtime, declaration, dependency-closure, and optional transform identities;
- direct export-to-summary references, with only export-local experimental
  status as the detailed form;
- hash-only proof and probe sidecar references.

`schemaStatus`, inline evidence, generator identity, trust/review labels, and
`compilerFactsProtocol` are rejected as unknown root fields. Summary IDs are
wire-local. Expansion derives an artifact-case ID from exact selection identity
and scopes operation/resource IDs to the exact case and export before entering
the semantic model. Summary renaming, JSON formatting, object-key order,
artifact-case order, and `closed`-list order therefore do not change the
canonical semantic digest.

The temporary wire records the runtime and types resolution branches as
provenance. They normalize to ordered `runtime` and `types` steps. Phase 7 still
owns the independently attested full resolver trace, exact runtime/declaration
target binding, and selection against a real `ResolvedImport`.

## Local knowledge and recursive leaves

`closed` names only an immediate sibling domain. Expansion maps wire omission
and closure into the Phase 5 lattice:

| Wire state | Normalized state |
| --- | --- |
| collection omitted and not closed | `Unknown` |
| non-empty collection and not closed | non-empty `Partial` |
| non-empty collection and closed | non-empty `Complete` |
| empty collection and closed | empty `Complete` |

A closed domain without its collection and an open empty collection are
rejected. Duplicate closed names and duplicate semantic positives are rejected.
Tuple items, object properties, choice alternatives, reactive/store
capabilities, resource states/capabilities, owner productions, and every call
claim retain independent local knowledge. Array element, minimum, and maximum
knowledge remain independent. Recursive expansion preserves an explicit
unknown leaf without opening known siblings and enforces a semantic recursion
limit of 32.

## Operations, ownership, resources, and guards

The wire covers all Phase 5 operation kinds and causal edge kinds. Trigger,
execution event, schedule, tracking, ownership, and cardinality decode as
independent axes. Cardinality preserves call/trigger/exact-resource scope,
unknown minimum/maximum, finite bounds, and `many`, so possible behavior is not
promoted to guaranteed behavior.

Owner source, current-owner requirement, child and cleanup requirements,
available child and cleanup capabilities, locally known owner production, and
lifetime are separate fields. Captured/created owners require exact owner
resources; created owners still require a separate positive production claim.
Resources carry exact kind, locally known states and capabilities, and an
independently optional lifetime.

Restricted guards cover signature, argument-count, literal, value-kind,
property, tuple-alternative, result-protocol, and exact artifact-case atoms.
Partitions without `otherwise` remain partial; `otherwise: true` closes the
finite partition. Branch operation sets may remain unknown by omission or are
complete when listed. Phase 5 validation rejects overlap, malformed
exhaustiveness, missing references, and causal cycles after expansion.

## Canonical digest rules

Phase 6 does not hash JSON. It expands wire mechanics and delegates to the
Phase 5 typed, length-delimited semantic digest under semantic-model version 1.
The digest includes exact package, manifest, runtime, declaration, transform,
closure, artifact-case, export, shape, call, operation, resource, guard,
ownership, cardinality, and stability meaning. It excludes temporary schema
version, formatting, object-key order, summary IDs, `closed` spelling/order,
sidecar paths/hashes, and receipt bytes. Artifact SHA-256 hex is canonicalized
to lowercase before case-ID derivation and semantic hashing.

## Structural and cross-field validation

The checked-in temporary JSON Schema rejects structural additions and pins the
full field vocabulary. Rust validation additionally enforces:

- 1 MiB document size, 16 KiB strings, 4 KiB package-relative paths, and no
  traversal outside the package root;
- semantic recursion depth 32 and JSON container depth 128;
- 1,024 entrypoints/artifact cases, 16,384 summaries, and 65,536 effective
  exports;
- 4,096 operations/resources, 8,192 edges, 256 guarded cases, and 256 atoms
  per expanded summary;
- a 1,000,000-node total expansion budget, preventing small shared-summary
  documents from causing multiplicative memory growth;
- dangling or unused summaries, invalid digests, invalid closure, duplicate
  identities/items, invalid paths, impossible owner/cardinality/resource/value
  combinations, missing graph references, cycles, and overlapping guards.

Summary references occur only from exports, so cycles are structurally
impossible. Expansion work is linear in the accepted expanded representation.

## Goldens and tests

Three complete goldens live under `benchmarks/package-contract-v2/phase6/`:

- `minimal-unknown.json` keeps omitted call domains unknown and carries both
  sidecar hash references;
- `signal-pair-complete.json` covers complete-positive and complete-negative
  call domains, operations and edges, a reactive resource, exact cardinality,
  locally closed owner production/capabilities, and a tuple with one unknown
  sibling leaf;
- `conditional-owned-effect.json` covers two exact artifact cases, transform
  identity, case/export experimental status, partial-positive resource states,
  callback provenance, tracked queued execution, created-owner production, and
  a closed guard partition.

Focused tests prove semantic round-trip idempotence for every golden, all four
knowledge states, summary/case/key/closure-order normalization equivalence,
deterministic semantic digests, false-closure rejection, invalid graph and
capability rejection, excluded fields and export overrides, path confinement,
document/collection/recursion limits, and temporary schema envelope constants.
The public-interface tests prove required format discrimination, stale receipt
rejection before normalization, successful normalization followed by
acceptance refusal, resolver authority, and content-addressed evidence storage.

## Fact producers and generated artifacts

No Type Facts producer/client/schema/protocol changed. No Solid compiler fork,
compiler pin, semantic trace, or compiler-facts protocol changed.

New development artifacts are:

- `schema/solid-reactivity-contract-v2.schema.json`;
- the three Phase 6 golden contract documents.

No bundled contract, generated analyzer snapshot, current generator output,
proof sidecar, probe sidecar, or acceptance receipt changed.

## Exact remaining open or uncertifiable cases

- No proposed closed claim is accepted. Proof replay, proof roots,
  closed-claim roots, receipt policy, and accepted typestate remain Phase 11.
- The sidecar fields are syntax- and digest-validated references only; their
  content is neither loaded nor treated as evidence in this phase.
- Exact artifact-case selection, full ordered resolver traces, independent
  runtime/declaration export targets, closure materialization, transform output
  binding, and dynamic-loading refusal remain Phase 7.
- Export targets temporarily normalize to the public export name in the exact
  runtime/declaration artifacts. A re-export or renamed target is not accepted
  until Phase 7 binds those targets independently.
- No generator can emit the replacement format yet, and no existing bundled or
  discovered contract is consumed as temporary v2. Those migrations remain in
  later phases.
- The runtime, async, cleanup/disposal, ownership, request/response, renderer,
  server-function transport, and experimental server-component evidence gaps
  listed in the Phase 5 conformance matrix remain open. Phase 6 represents
  those exact leaves but does not establish their premises.

## Verification

Focused iteration:

- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib`
  — 39 passed;
- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib`
  — 168 passed;
- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test contract_interface`
  — 6 passed;
- `cargo +1.97 clippy --manifest-path rust/Cargo.toml -p solid-facts-backend --lib -- -D warnings`
  — passed;
- `cargo +1.97 fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `git diff --check`, `jq empty schema/*.json`, and
  `bun scripts/dialect-manifests.mjs validate` — passed; two dialect manifests
  validated;
- the three Phase 6 goldens were also validated against the checked-in schema
  with the locally installed AJV validator — all valid.

Final authority:

- `make verify` — passed all 26 timed steps in 68.98 seconds. This included Go
  formatting/vet/race tests, dependency lock verification, workspace Clippy,
  both backend and WASM dialect feature checks, compiler identity, armed Rust
  workspace/process tests, fresh checker build, coverage, ownership, performance,
  CLI tests, the Phase 0 baseline guard, TypeScript oracle tests/gate, obligation
  audit, schema/manifest lint, bundled-contract conformance, and registry pin
  verification.

## Handoff

- branch: `codex/phase6-temporary-wire-schema-v2`;
- implementation commit and solid-checker PR: recorded after the first push;
- no upstream Solid PR was created.
