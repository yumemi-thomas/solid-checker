# Phase 17 completion report — temporary version-2 convergence

Date: 2026-08-29
Branch: `codex/phase17-temporary-version2-convergence`

## Outcome

Phase 17 makes temporary schema version 2 the sole active main-document format
throughout solid-checker without performing Phase 18's public stable-version
cut. A permanent repository audit covers 130 main documents, 73 exact
byte-bound receipts, 69 independently versioned neighboring documents, 15
producer/consumer source owners, and 579 active JSON files.

The retired legacy-v1 schema is deleted. No legacy main decoder, JavaScript
semantic normalizer, generator, runtime-closure scanner, or Rust public
contract type remains. The immutable Phase 0 artifacts retain their historical
measurements and hashes, but their script can no longer reconstruct or execute
the retired decoder.

## Convergence and version namespaces

`scripts/package-contract-v2-phase17.mjs` is the checked authority. It requires
every active `solid-reactivity-contract` main to use `schemaVersion: 2` and
`semanticModelVersion: 1`, rejects the legacy-v1 structural shape and
`schemaStatus`, and verifies every receipt against the exact raw sibling bytes.
It also pins the exclusive Rust decode/normalize owners and the intentionally
thin Node orchestration boundaries.

The audit treats each adjacent protocol independently. Receipts, proposal
plans, proof transcripts, accepted catalogs, evidence sidecars, runtime
resolution, rules, and ownership manifests remain version 1 where specified;
runtime-probe, bundle-index, runtime-lock, dialect-manifest, and temporary main
documents remain version 2. No global schema-version replacement occurred.
Gate-cache and registry-integrity memo formats intentionally moved from 1 to 2
because the convergence audit widened their input closure.

The ecosystem workflow now watches the live CLI script directory, the
universal schema check points at
`schema/solid-reactivity-contract-v2.schema.json`, and active authoring
documentation describes receipt-only temporary-v2 acceptance rather than the
retired name-based legacy workflow.

## Frozen semantic identity

Semantic-model version 1 is frozen independently of the wire version. Its
canonical digest uses SHA-256 with the domain separator
`solid-checker:normalized-package-contract`. Strings and bytes use unsigned
64-bit big-endian byte lengths, sequences use unsigned 64-bit big-endian item
counts, fixed integers are big-endian, and options, variants, booleans, and
knowledge states carry explicit tags. Normalized semantic order and exact
artifact identity are hashed; JSON formatting, key order, summary IDs, receipt
bytes, evidence paths, and wire schema version are not.

The checked golden normalized proposal hashes to
`sha256:23c3aef34b18c809cbfe185cb53ed4b37275ab6486da190b37f4e18d8291c2b9`.
The Phase 18 wire renumbering must preserve this value for unchanged semantics.

## Tests and adversarial coverage

The new Phase 17 tests accept a temporary-v2 main only with its exact
byte-bound receipt, reject legacy structural and explicit schema-v1 mains,
reject stale receipt wire digests, preserve a catalog's independent version 1,
and run the complete repository audit. The Rust golden test freezes the model
version, hash family, domain separator, canonical encoding, and exact digest.

Focused verification completed during implementation:

| Command | Result |
| --- | --- |
| focused Phase 17, Phase 0, gate-cache, pin-memo, and cache-workflow Vitest suite | 5 files, 57 tests passed |
| focused semantic digest golden test | 1 passed |
| `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib` | 190 passed |
| `bun scripts/package-contract-v2-phase17.mjs` | 130 mains, 73 receipts, 69 independent documents, 15 owners, 579 JSON files passed |
| `SOLID_CHECKER_GATE_CACHE=0 make verify` | passed in 207.17 seconds; both gate caches disabled and 7 registry pins checked live |

The full authority also passed 94 fixture projects with 542 findings, 161
TypeScript-oracle cases with 41 keystones, 289 ownership cases with 465 ledger
rows, 39 generator fixtures, 24 receipt-issued bundle cases, compiler and Type
Facts identities, both dialect and WASM feature builds, performance
certification, packaging, and conformance.

## Type Facts, compiler facts, and generated artifacts

No Type Facts producer, Rust client, normalized consumer, schema, protocol,
build identity, or binary changed. No Solid compiler semantic fact, compiler
pin, identity notice, or conformance artifact changed.

No temporary-v2 main, receipt, bundle, fixture contract, or generated report
was regenerated. The obsolete legacy schema was deleted; source and test
fixtures were updated only where they named the retired tools. Cache format
constants changed deliberately, but ignored cache contents are not artifacts.

## Exact remaining open or uncertifiable cases

- Every generated ecosystem proposal remains unaccepted until selected closure
  claims pass all proof families and a matching receipt is issued.
- The Phase 16 set of 60 full-row and 1,458 localized artifact-case refusals
  remains uncertifiable; all open call domains and 13 recursive unknown leaves
  remain local.
- The Phase 13 browser DOM/hydration, request and transport integration, user
  serialization, dynamic payload/target/selection, TypeScript-owned
  declaration, and unstable-protocol domains remain open.
- Wildcard-only surfaces still require a finite census. Missing or non-file
  targets, unresolved callable kind, external export-all without accepted
  dependency semantics, closure hazards, stale receipts, and unsupported
  artifact shapes continue to fail closed.
- The Phase 16 process peak remains an upper bound rather than a retained-
  analyzer-heap measurement.

Phase 17 claims completion of plan items 197–202 and internal temporary-v2
convergence. It does not claim complete package semantics and does not perform
the Phase 18 stable schema-version cut.

## Handoff

- Branch: `codex/phase17-temporary-version2-convergence`
- Implementation commit and pull request: recorded after the green handoff
  commit is pushed.
