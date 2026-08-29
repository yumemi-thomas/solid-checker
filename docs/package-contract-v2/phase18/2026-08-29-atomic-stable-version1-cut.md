# Phase 18 completion report — atomic stable version-1 cut

Date: 2026-08-29
Branch: `codex/phase18-atomic-stable-version1-cut`

## Outcome

Stable public `schemaVersion: 1` is now the only live package-contract main
format. The required `format: "solid-reactivity-contract"` discriminator
distinguishes it from the retired legacy-v1 shape, and the sole Rust decoder
rejects both legacy-v1 and temporary-v2 inputs. The stable structural authority
is `schema/solid-reactivity.schema.json`; the temporary schema path and
version-suffixed Rust and CLI owners are retired.

The atomic cut re-emitted all 130 active mains and reissued all 73 acceptance
receipts. Every new receipt binds its sibling main's exact stable bytes.
First-party bundle issuance also regenerated proof transcripts, proof roots,
and wire-only main sidecar hashes whose bytes depend on the stable encoding.
Canonical semantic digests, artifact roots, closure roots, closed-claim roots,
verifier identities, and semantic-model version 1 remain unchanged.

## Stable boundary and invariants

`solid-facts-backend::contract_document` owns parsing, structural and
cross-field validation, private wire mechanics, normalization, and stable
encoding. `inferred_contract` and the proposal/proof workflow use that owner;
analysis still receives only receipt-validated normalized semantics through
`AcceptedContractIndex`. Summary IDs, `closed` arrays, aliases, omission rules,
and schema mechanics do not cross the boundary.

The Phase 18 repository authority is
`scripts/package-contract-phase18.mjs`. It inventories 130 stable-v1 mains, 73
exact byte-bound receipts, 69 independently versioned neighboring documents,
15 producer/consumer source owners, and 579 active JSON files. It rejects
`schemaStatus`, stale receipt hashes, temporary-v2 mains, legacy-v1 structural
mains, retired owner paths, or an unregistered JavaScript main reader.

Version namespaces remain independent. Runtime-probe documents and contract
review documents remain version 2; receipts, accepted catalogs, proposal plans,
proof transcripts, and evidence sidecars remain version 1; dialect manifests
and the runtime lock retain their existing versions. Bundle indexes and the
generator corpus now use stable package-contract-specific format names at
version 1. Gate-cache and registry-integrity memo formats moved from 2 to 3 so
pre-cut verdicts cannot cross the changed input closure.

## Re-emission and generated artifacts

- Renamed the public schema to `schema/solid-reactivity.schema.json` and set
  its main envelope constant to 1.
- Renamed the Rust wire owner to `contract_document`, the inferred adapter to
  `inferred_contract`, and the Node producer to
  `generate-package-contract.mjs`.
- Re-emitted 130 main documents across Phase 6/14 authorities, generator
  fixtures, accepted analyzer fixtures, backend fixtures, and both first-party
  bundle locations.
- Reissued 73 receipts. A comparison with `origin/main` proves all 130
  normalized JSON payloads differ only in the main schema number and wire-only
  sidecar digests, while all 73 receipts preserve semantic, artifact, closure,
  closed-claim, and verifier identity.
- Regenerated all 24 first-party bundle cases in both physical locations,
  their four bundle indexes, 39 generator-corpus fixtures, and the Phase 16 and
  418-row ecosystem JSON/Markdown reports.
- Refreshed cache format identities. Dialect manifests and
  `pkg/contracts/bundled/runtime-lock.json` were checked and remain byte-
  unchanged because neither embeds a main-wire hash and each has an independent
  version namespace.
- Rebuilt debug, release, bundle, Phase 16, session-benchmark, and packaged
  native checker binaries. These build products are ignored artifacts; no
  checked binary is part of this change. The existing Type Facts binary stamp
  already matched its unchanged sources.

## Tests and verification

Focused red/green work established the stable rejection and digest behavior
before the repository-wide cut. The initial Phase 18 Vitest failed because the
new authority module did not yet exist; the initial backend library run then
identified four remaining temporary-version test fixtures. After the cut:

| Command | Result |
| --- | --- |
| focused Phase 18, ecosystem classifier/content, cache-workflow, and CLI contract-workflow Vitest | 5 files, 42 tests passed |
| `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib` | 86 passed |
| armed backend `contract_interface`, `contracts_process`, and `dialects_process` tests | 50 passed |
| `bun scripts/package-contract-phase18.mjs` | 130 mains, 73 receipts, 69 independent documents, 15 owners, 579 JSON files passed |
| origin/main stable-cut comparison | 130 semantic-preserving main re-emissions and 73 exact receipt reissues passed |
| uncached `make contract-corpus` | 39 fixtures; 5 exact refusals, 14 local refusals, 40 retained cases passed |
| uncached `make contract-conformance` | 24 cases in both locations and 7 live registry pins passed |
| uncached `make tsc-oracle` | 161 cases and 41 keystones passed |
| uncached `make ownership-gate` | 289 cases and 465 complete ledger rows passed |
| uncached fresh-debug coverage | 94 projects and 542 findings passed |
| uncached `make ecosystem-benchmark` | 418 rows: 40 complete proposals, 318 partial proposals, 60 fail-closed rows; thresholds passed |
| `make phase16-check` | 85.65% ecosystem and 94.16% Solid Primitives generatable; passed |
| uncached `make verify-performance` | passed; 1.91x export scaling and all latency/payload ceilings held |
| `SOLID_CHECKER_GATE_CACHE=0 make verify` | passed in 199.65 seconds on the final implementation tree; both gate caches disabled and 7 registry pins checked live |

The first sandboxed ecosystem attempt produced 418 installation failures with
Bun `EPERM` before generation. A one-row deterministic repro produced one
complete contract when granted normal package-install permissions, proving the
failure was sandbox infrastructure rather than package semantics. The full
uncached run was then repeated with those permissions and produced the expected
40/318/60 boundary above; its reports replace the non-authoritative failed run.

## Type Facts and compiler facts

No Type Facts producer, Rust client, normalized consumer, schema, protocol,
toolchain identity, build ID, or focused fixture changed. No Solid compiler
semantic trace, recording hook, serialization, test, fork pin, identity
document, notice, or conformance authority changed. Phase 18 exposed no new
semantic premise; it changed only public main-wire identity and the exact proof
artifacts that bind those bytes.

## Exact remaining open or uncertifiable cases

- Every generated ecosystem proposal remains unaccepted until the selected
  closure claims pass every required proof family and a matching receipt is
  issued. Coverage and probe non-observation never grant closure.
- The Phase 16 boundary remains 60 full-row and 1,458 local artifact-case
  refusals. Across the 358 emitted proposals, all 4,788 exports retain open
  call domains and recursive uncertainty remains local; the measured wire
  corpus records 43,106 open domain/recursive observations.
- The Phase 13 browser DOM/delegation/hydration, request and transport
  integration, user serialization, dynamic payload/target/selection,
  TypeScript-owned declaration, and unstable-protocol domains remain open.
- Wildcard-only surfaces still require a finite census. Missing or non-file
  targets, unresolved callable kind, external export-all without accepted
  dependency semantics, closure hazards, stale receipts, and unsupported
  artifact shapes continue to fail closed.
- The Phase 16 process peak remains a whole-process upper bound, not a retained
  analyzer-heap measurement.

Phase 18 claims completion of plan items 203–212 and the atomic stable public
version-1 cut. It does not claim complete package semantics.

## Handoff

- Branch: `codex/phase18-atomic-stable-version1-cut`
- Implementation commit: this report's implementation commit (recorded after
  the final green authority)
- Pull request: recorded in the follow-up handoff commit after creation
