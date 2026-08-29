# Phase 16 completion report — corpus, compactness, and performance

Date: 2026-08-28
Branch: `codex/phase16-corpus-compactness-performance`

## Outcome

Phase 16 runs the temporary-v2 proposal generator over the exact 418-row
ecosystem corpus and measures the existing 24 receipt-issued first-party cases
through the real proof and accepted-load boundaries. The gate reaches 85.65%
ecosystem and 94.16% Solid Primitives generatability without accepting a
generator claim, treating probe absence as proof, or weakening any closure
requirement.

The generator now keeps finite explicit entrypoints when a package also
publishes wildcard patterns, catches failures at the exact artifact case, and
greedily replays Rust's merge boundary when one proposal contradicts its
siblings. Rejected cases receive a stable sidecar refusal record. Independent
cases remain in a partial proposal; a package with no certifiable case still
fails closed. The benchmark classifies partial proposals separately from
structurally complete proposals.

## Corpus and preservation

| Corpus | Rows | Complete proposals | Partial proposals | Refused rows | Generatable |
| --- | ---: | ---: | ---: | ---: | ---: |
| Official ecosystem | 418 | 40 | 318 | 60 | 85.65% |
| Solid Primitives | 291 | 6 | 268 | 17 | 94.16% |

Published `solid-js@2.0.0-rc.3`, `@solidjs/signals@2.0.0-rc.3`, and
`@solidjs/web@2.0.0-rc.3` are required generatable rows. The report also checks
all eight ecosystem families, 39 synthetic generator fixtures, 16 Solid 2
conformance rows, and all 24 previously receipt-issued artifact cases.

“Complete” means generator orchestration refused no artifact case. It does not
mean the proposal is proved or accepted. The live corpus contains zero fully
proved proposals; proof verification and a matching receipt remain the only
path into ordinary analysis.

## Exact refusal boundary

The 318 partial rows retain independent known cases and record 1,458 exact
artifact-case refusals. The 60 full-row refusals consist of:

- 20 accepted-dependency contract obligations;
- 14 unresolved export-kind censuses;
- 8 packages with no runtime ESM export surface;
- 2 missing exact package-export identities;
- 1 unresolved parameter-behavior case;
- 15 unresolved or unsupported exact artifact shapes, including wildcard-only
  public censuses, missing local declaration closure files, and published
  runtime targets that do not resolve to files.

All nine call domains remain locally open for the 4,788 emitted proposal
exports because coverage is not closure proof. Recursive uncertainty occurs on
13 exact leaves and does not contaminate siblings. The Phase 13 report retains
36 separately named RC.3 open rows, including browser DOM/hydration timing,
request/transport integration, user serialization, dynamic payload/target/
selection leaves, the server-functions declaration self-error owned by
TypeScript, and unstable frames protocol details. Exact rows are in
`benchmarks/package-contract-v2/phase16/refusals.json`.

## Compactness

| Material | Count | p50 bytes | p95 bytes | max bytes |
| --- | ---: | ---: | ---: | ---: |
| Ecosystem canonical proposal main | 358 | 1,599 | 4,813 | 43,055 |
| Ecosystem proposal plan | 358 | 49,172 | 303,349 | 3,676,782 |
| Accepted canonical main | 24 | 3,342 | 16,202 | 21,538 |
| Accepted raw proof evidence | 24 | 348,386 | 3,122,474 | 3,209,546 |
| Acceptance receipt | 24 | 669 | 669 | 669 |

The accepted report also records pretty-main, normalized-semantic debug,
proposal, bytes-per-export, and bytes-per-operation distributions. Canonical
accepted mains pass the 8 KiB p50, 32 KiB p95, and 1 MiB maximum gates. No new
compression transform was added: compression is permitted only when decoding
produces identical normalized semantics and preserves all proof identities.

## Performance and ordinary analysis

The release corpus records generation at 1,568 ms p50, 25,439 ms p95, and
434,856 ms maximum per row. The current temporary-v2 probe driver runs its
deterministic witness in a fresh process, realm, and module instance at 20.13
ms p50, 21.16 ms p95, and 21.73 ms maximum per isolated session. This is an
execution-cost measurement, not semantic acceptance authority.

For the accepted 24-case corpus, proposal/proof-input construction is about
1.92 ms p50/14.88 ms p95; proof verification and receipt issuance is about
22.74 ms p50/206.06 ms p95. Loading all 24 cases is about 15.89 ms p50/16.15 ms
p95 across 25 iterations. Normalized export lookup is 31 ns p50/32 ns p95
across 250 iterations.

The measured 31,232 KiB peak-RSS delta uses `getrusage` over the whole benchmark
process and includes checked-corpus construction and accepted loading. It is an
upper bound, not a retained-heap measurement. The historical Phase 0 isolated
probe distribution remains an execution-envelope reference only and is
explicitly not current semantic acceptance authority.

Ordinary queries receive only receipt-validated normalized semantics through
`AcceptedContractIndex`. A source gate and a drop-before-query regression prove
that the analyzer-facing path retains zero raw sidecar bytes and performs no
package code execution, network access, or query-time file reads.

## Tests and verification

Focused checks completed while implementing the slices:

| Command | Result |
| --- | --- |
| focused ecosystem, Phase 16, and CLI workflow Vitest suite | 12 files, 180 tests passed |
| `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib` | 85 passed |
| `bun run --cwd packages/cli test` | 4 files, 42 tests passed; TypeScript check passed |
| targeted backend Clippy (`--lib --bins`) | passed with `-D warnings` |
| `make ecosystem-benchmark` | passed; 418 rows, 40 complete, 318 partial |
| `make phase16-report && make phase16-check` | passed; 85.65% / 94.16% milestones |
| `make ecosystem-benchmark-test` | 10 files, 173 tests passed, including heartbeat and exact sentinel-family matrix coverage |
| Ruby YAML parse of `.github/workflows/ecosystem-benchmark.yml` | passed |
| `make verify` | passed again in 164.44 seconds with the family-sharded, classification-preserving CI configuration, including the generator corpus |

The complete gate passed Go formatting, vet, and race tests; workspace Clippy;
backend and WASM feature configurations; compiler identity and Type Facts stamp
checks; 61 facts, 85 backend, 189 IR, 35 Type Facts, and all dialect/process
tests; 94 fixture projects with 542 findings; the 161-case TypeScript oracle and
41 keystones; ownership with 289 cases and 465 ledger rows (none pending);
performance certification; CLI and WASM tests; seven obligations and eleven
closures; 18 script files with 108 tests; 24 receipt-issued bundle cases in
both physical locations; all seven package pins; and composed conformance.

Focused coverage pins finite-plus-wildcard entrypoint locality, independent
merge retention, integrity handoff, artifact-case refusal classification,
proposal content/size measurement, generatability thresholds, report
determinism, all 24 accepted cases, and query closure after raw inputs are
dropped.

The CI follow-up also reproduced and closed the two deterministic failures:

| Command | Result |
| --- | --- |
| `cargo +1.97 check --manifest-path rust/Cargo.toml -p solid-facts-backend --target wasm32-wasip1-threads` | passed; the non-Unix RSS path compiles without `getrusage` |
| `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib` | 85 passed |
| `SOLID_CHECKER_GATE_CACHE=0 make contract-corpus` | 39 fixtures passed: 5 full refusals, 14 local refusals, 40 retained cases |
| `bun scripts/ecosystem-benchmark/run.mjs --sentinel --timeout 120` (with fresh checker and Type Facts binaries; reports redirected to `/tmp`; local default resolved to the workflow's 8 workers) | passed; 23 probes, 5 complete contracts, 7 partial; the pinned timeout probe remained `timeout during generate` |
| focused workflow and heartbeat Vitest | 2 files, 33 tests passed; heartbeat scheduling, deterministic progress, and cancellation are pinned |

Five ecosystem sentinel attempts received an external `SIGTERM` without
reaching a benchmark assertion. Runs ended after varying 2m53s, 4m0s, and 5m11s
overall, falsifying a stable job-duration ceiling. The fifth log contained
heartbeats at 30, 60, and 90 seconds immediately before termination, also
falsifying silence as the cause. A 60-second local run was rejected because it
changed four unrelated Kobalte, Motion, and Solid RC.3 classifications to
timeouts.

The final workflow therefore shards the pinned set by its eight manifest-owned
families. Every shard keeps the classification-preserving 120-second timeout
and runs at most 1–6 related probes with four workers; a data-driven test proves
that every sentinel ID belongs to exactly one listed matrix family. Per-family
reports are uploaded independently, and a fast aggregate job retains the
stable `sentinel` verdict and requires every shard to pass. The progress
heartbeat remains operational-only, is canceled in `finally`, and never enters
results, reports, digests, or threshold decisions. The benchmark runner and
`make ecosystem-sentinel` retain the 300-second operator default.

## Type Facts, compiler facts, and generated artifacts

No Type Facts producer, Rust client, normalized consumer, schema, protocol,
toolchain identity, or checked binary changed. No Solid compiler semantic-fact
code, compiler pin, identity notice, or conformance artifact changed.

Generated source-controlled reports changed under `benchmarks/ecosystem/` and
`benchmarks/package-contract-v2/phase16/`; `rust/Cargo.lock` also records the
backend's direct `libc` use for peak-RSS measurement. The CI follow-up migrated
the 39-fixture generator corpus to the Phase 16 partial-proposal behavior: 8
former whole-package refusals now carry main, proposal-plan, and localized
refusal snapshots, while 5 remaining full refusals pin the aggregate envelope.
The temporary main schema version, bundled contracts, acceptance receipts,
compiler identity, and Type Facts build identity are unchanged.

## Exact remaining open or uncertifiable cases

- Every generated ecosystem proposal remains unaccepted until the selected
  closed claims pass every proof family and receipt issuance.
- The 60 full-row and 1,458 localized artifact-case refusals above remain
  uncertifiable; the checked refusal JSON is the exact machine-readable list.
- All open proposal call domains and 13 recursive unknown leaves remain local;
  missing evidence never becomes complete-negative knowledge.
- The Phase 13 RC.3 browser, request/transport, serialization, dynamic-leaf,
  TypeScript-owned declaration, and unstable-protocol domains remain open.
- Wildcard-only surfaces still require an explicit finite entrypoint census.
  Missing/non-file targets, unresolved callable kind, external export-all
  without accepted dependency semantics, closure hazards, stale receipts, and
  unsupported artifact shapes continue to fail closed.
- The memory figure is a reproducible process peak, not retained analyzer heap;
  a dedicated allocator-backed retained-heap benchmark remains open if that
  stronger distinction is required.

Phase 16 claims completion of plan items 186–196 and the stated exit gates, not
complete semantic knowledge of every package export.

## Handoff

- Branch: `codex/phase16-corpus-compactness-performance`
- Implementation commit: `35ab4d36` (`feat: complete package contract phase 16
  gates`)
- CI correction commit: `6b408374` (`fix: make phase 16 gates portable and
  complete`)
- Pull request: <https://github.com/yumemi-thomas/solid-checker/pull/58>
