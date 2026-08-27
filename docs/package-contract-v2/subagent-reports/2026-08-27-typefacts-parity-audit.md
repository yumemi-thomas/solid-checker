# Type Facts repatriation parity audit

Date: 2026-08-27

Scope: read-only audit of `solid-ts-facts@92c53392388518d69ef27220729f5c061479deed` and the current `solid-checker` Type Facts consumers. This defines the proof required for a source-location/build migration with no Type Facts protocol or semantic-fact delta.

## Frozen authority

- External source: `/Users/thomas/Documents/Github/solid-ts-facts`, clean at `92c53392388518d69ef27220729f5c061479deed`.
- Checker dependency: `rust/Cargo.toml` and `rust/Cargo.lock` resolve the same exact commit.
- Go toolchain observed: `go1.26.5 darwin/arm64`; module declaration requires Go 1.26.
- Rust toolchain observed: Cargo `1.97.1`; both workspaces require Rust 1.97.
- Lifecycle schema: `1`.
- active Wire table schema emitted/required by the client: `17`.
- startup handshake protocol: `2`.
- schema identity: `sha256:9a217ca6aa3b147f84cd356df069259ecd548328ab0c48c83109832d1cbedeb9`.
- codec-limits document SHA-256: `3f511a4bf87d91fcffa021a21942458ce413771153e17da49c383d9bbd4beff0`.
- Golden hashes:
  - module graph request: `ea908beca2c149b43a049583d748c31f1be26d8bdbb7b279bb5503be779ec527`;
  - module graph response: `0fa3434f951e9a37705c32f14b480fe523c7304571f71cc60480295b2d4a3437`;
  - lifecycle request: `bfda90e8cc17a0129dce70e990fa8272fead6bb0272681960afdd16cec12a578`;
  - lifecycle response: `8494ad7a589611e139af1f89a488789ea58cede9a69e975ba9ae0a56a8c2ff62`;
  - packed transition: `b91272212c1cbff9743fdccee40acfe7bac1c4aaa432ad7bed6230e36a53fbcf`.
- Import inventory: 205 tracked files, 1,570,264 tracked blob bytes, 163 Go tests, 18 Go benchmarks, 32 Type Facts testdata files, and the Rust unit/process/golden/public-API suites.

These values must be written to a checked-in baseline record before changing the dependency or build path.

## Findings and blockers

### P0 — Raw response-byte identity is not a valid oracle

Every materialized analysis response contains `timings` with nanosecond values (`requestDecodeNs`, `analyzeNs`, phase durations). Two executions of the unchanged external binary therefore need not emit the same complete frame. The bootstrap requirement to compare “byte-identical responses” must mean byte-identical deterministic semantic projections:

1. decode with the frozen strict codec;
2. retain and compare the existence and structural fields of timing data, but remove its numeric durations;
3. re-encode the remaining response canonically;
4. require those bytes to match exactly;
5. compare performance measurements separately.

Do not simply compare decoded object equality: canonical re-encoding is needed to retain the proof of row order, optional-field omission, enum tags, transition bytes, state tokens, source arenas, and deterministic CBOR.

The handshake itself can and should be byte-identical when both builds are supplied the same explicit parity build ID.

### P0 — No external-versus-local transcript A/B harness exists

The repository has strong golden, retained-vs-fresh, process, crash/replay, and cancellation tests, but no tool that launches two independently built producers with one identical transcript and compares their responses. T11 is therefore not currently executable.

Add a bootstrap-only or permanent parity driver before switching the dependency. Its fixed transcript must use one shared absolute fixture path and stable request IDs and cover:

- handshake;
- `open` generation 1;
- `sources`;
- `modules`, including resolved, unresolved, paths-aliased, project-reference, package, and symlink rows;
- full `analyze` with all demand flags represented;
- unchanged `analyze` reuse;
- `symbols` closure/reference evidence;
- leaf update followed by delta analysis;
- shape/global/config-changing update followed by the fail-closed/full path;
- deletion and restoration;
- `close`.

Cancellation ordering, producer death, and replay cannot be usefully certified as deterministic transcript bytes; retain their behavioral tests as separate obligations.

### P0 — The production switch must be atomic, but the parity test should deliberately cross-pair

Production must never leave an external producer with a local client or vice versa. Before the atomic switch, however, compatibility should be proved with four test-only pairings under one explicit build ID:

1. external client + external producer;
2. external client + imported producer;
3. imported client + external producer;
4. imported client + imported producer.

The two cross-pairs are the strongest proof that relocation did not accidentally change the seam. They should run only in the parity job; the committed build and ordinary tests must use local/local.

### P0 — Codec limits are not a fourth handshake field

The current handshake contains exactly `protocol`, `schemaHash`, and `buildId`. `typefacts-codec-limits.json` is not included in the schema hash. Go tests check the JSON against Go constants, and Rust has matching enforcement, but the startup handshake does not independently attest the limits.

Adding a handshake field would violate the no-protocol-delta rule. The smallest correction is to make the existing build identity derive from a source manifest that includes the codec-limits JSON and both codec implementations, then compile the same identity into the Go producer and Rust client. Build the external parity pair with that same explicit value. This binds codec limits without changing a frame or schema.

### P0 — Relocation requires path-only source edits that must be isolated

The proposed destination changes relative paths and Go import paths:

- Rust `include_bytes!` and golden paths currently assume `crates/typefacts`; under `rust/crates/typefacts` they need one additional parent traversal.
- Rust `repository_root()` helpers currently walk two parents and assume producer/testdata live at the external root.
- Go packages currently import `github.com/yumemi-thomas/solid-ts-facts/internal/typefacts`; moving private code under `apps/solid-typefacts` requires a mechanical import-path change.
- Rust process tests and benchmarks build `./cmd/solid-typefacts` and look in `internal/typefacts/testdata`; both paths change.
- Go pin/linkname tests assume root-relative `go.mod` and `shims` paths.

Keep the exact-history import and `git mv` relocation separate from one small “path adaptation only” commit. A destination manifest must map every imported source blob to its external Git blob ID; only the enumerated path adapters may differ. Any other source-content difference blocks parity.

### P1 — The new build stamp must prevent stale same-build-ID binaries

Today the external revision is both the Rust dependency identity and the producer build source, while `bin/solid-typefacts.buildinfo` records `revision=<sha> build-id=<id>`. Once the git pin disappears, `buildId=dev` alone would accept a stale producer.

The replacement stamp must hash, with sorted path names and unambiguous length framing:

- all Go producer/internal sources and root `go.mod`/`go.sum`;
- every shim source, `go.mod`, and `go.sum`;
- `rust/crates/typefacts` sources and Cargo manifest;
- lifecycle schema and codec-limits JSON;
- protocol goldens;
- TypeScript-Go revision;
- Go and Rust toolchain identities relevant to the build;
- requested release/development build ID;
- the manifest format version.

Use this digest in the cache stamp and incorporate it into the existing handshake `buildId` value. Build producer and Rust client from the same generated identity. Do not hash `bin/`, target directories, test outputs, absolute checkout paths, mtimes, or the stamp itself.

The build script must compute the digest before deciding to skip, delete the stamp before compiling, build to a temporary file in the destination directory, and atomically rename the successful binary and then the successful stamp. A failed build must never leave a new binary beside an old-valid stamp.

### P1 — Existing tests are broad, but parity needs baseline artifacts rather than pass/fail alone

The external suite already covers:

- deterministic/strict CBOR, unknown/duplicate fields, codec limits, and Go/Rust goldens;
- all currently active semantic domains across the process seam;
- full, delta, and reuse transitions;
- retained-vs-fresh materialization, canonical ordering, sparse transition patching, and transactional rollback;
- cancellation before commit and while work is active;
- ordered pipelining;
- producer crash, restart, update replay, and failure between lifecycle steps;
- stale/foreign locations, stale generations, malformed transitions, contradictory enum/symbol states, UTF-8 boundaries, aliases/cycles, global augmentation, config alternation, deletion/restoration, non-durable files, project references, symlinks, and unresolved imports;
- memory ownership and retained-session benchmarks.

What is missing is a checked-in before/after conformance report containing command, toolchain, binary/build identity, corpus identity, exact pass counts, semantic transcript hashes, finding snapshot hashes, and performance/memory samples. Without that record, a green rerun cannot establish that the intended old authority was tested.

### P1 — Binary byte identity is neither guaranteed nor required

Go build metadata and source-root changes may make executables differ even with identical behavior. Record binary SHA-256 and size for audit, but do not make identical executable bytes an exit condition. The required identities are source manifest, handshake, deterministic semantic response, behavior, findings, and bounded resource use. A binary difference with all of those equal is an identity-only relocation result and should be documented.

### P1 — Current performance gates are too loose for relocation parity

`verify-performance.mjs` has shared-runner ceilings and `compare-performance.mjs` permits 1.35x by default. Those are appropriate ongoing regression guards, not a no-code-change relocation proof. Use the exact same release checker binary for both Type Facts producers, alternate old/new on the same generated corpus, and set the relative limit to 1.10 for the bootstrap. Require no statistically credible regression in producer benchmarks; investigate any median over 5%, even if the 10% hard gate passes.

For retained physical memory, the existing 1,200 MiB ceiling is also only a gross guard. Compare at least three alternating old/new 5,000-file measurements; require imported median physical footprint no greater than `max(old * 1.05, old + 25 MiB)`. Preserve the absolute 1,200 MiB bound.

### P1 — CI and cache assumptions are revision-shaped

`.github/actions/typefacts/action.yml`, `scripts/build-typefacts.sh`, performance merge-base builds, package/release workflows, and documentation currently extract the external `rev`, clone/fetch a repository, and cache by that revision. All must move in the same commit. Gate-cache currently includes the binary and buildinfo; that remains safe only if every producer-dependent gate first runs the local build and buildinfo contains the complete source digest.

### P2 — Historical documents need labels, not semantic rewrites

Preserve all Type Facts ADR numbers and performance reports. Update active reproduction paths and mark their imported source revision, but do not rewrite historical measurements or old architectural descriptions to look current. The duplicate historical ADR basename `0021-*` must remain unchanged; renumbering would falsify provenance.

## Before/after proof matrix

| Domain | Existing authority | Before artifact | After obligation |
|---|---|---|---|
| Source provenance | external Git commit | blob-ID/path manifest | every non-adapter destination has same blob; adapters explicitly diffed |
| Protocol identity | Go/Rust constants and schema | protocol 2, schema 1/table 17, schema SHA, codec SHA | identical constants/hashes; no schema or frame-shape diff |
| Codec/goldens | Go/Rust golden tests | five SHA-256 values above | unchanged bytes and round-trip encodings |
| Handshake | process refusal test | canonical frame hash with explicit parity build ID | byte-identical; mismatch on protocol/schema/build still refused before request |
| Semantic wire | retained/full/delta/golden tests | normalized hashes for fixed transcript | identical canonical bytes after only numeric timings are removed |
| Cross-language compatibility | Rust process suite | old/old and old/new results | all four client/producer pairings pass |
| Lifecycle | command and Rust session process tests | ordered open/update/analyze/sources/modules/symbols/close result | same generations, tokens, affected sets, close/drop behavior |
| Cancellation | queue and transaction tests | pass/fail plus elapsed bound | no commit/leak/stranded update; service stays usable |
| Restart/replay | Rust crash injection tests | generation and resulting table/finding hashes | same update replay and recovery at each injected death point |
| Incremental correctness | retained-vs-fresh adversarial oracles | canonical table hashes per generation | exact equality at every generation |
| Checker process seam | transport/sessions tests | pass counts and output hashes | identical results against local/local |
| Checker findings | coverage and ownership gates | frozen snapshots/report hashes | no unexplained finding, span, severity, or uncertifiable change |
| Contracts | contract process/corpus/differential gates | status/closure hashes | unchanged statuses and evidence |
| Performance | Go benchmarks + checker session bench | raw samples, medians, response bytes | <=1.10 hard relative gate; >1.05 investigated |
| Memory | structural tests + 5k process tree | three old samples, heap/physical metrics | structural tests equal; <=5% or +25 MiB and <=1,200 MiB |
| Packaging | release build and launcher tests | executable names, build IDs, package inventory | local binary packaged for every target; no external checkout needed |

## Concrete command sequence

Use explicit directories; do not overwrite the checked-in `bin/solid-typefacts` while establishing parity.

```sh
CHECKER=/Users/thomas/Documents/Github/solid-checker
EXTERNAL=/Users/thomas/Documents/Github/solid-ts-facts
EXTERNAL_REV=92c53392388518d69ef27220729f5c061479deed
PARITY_ID=typefacts-parity-92c53392388518d69ef27220729f5c061479deed
OLD_BIN=/tmp/solid-typefacts-92c5339
NEW_BIN=/tmp/solid-typefacts-local
```

### 1. Freeze source and identity

```sh
git -C "$EXTERNAL" diff --quiet
git -C "$EXTERNAL" diff --cached --quiet
test "$(git -C "$EXTERNAL" rev-parse HEAD)" = "$EXTERNAL_REV"
git -C "$CHECKER" status --short

git -C "$EXTERNAL" ls-tree -r --full-tree "$EXTERNAL_REV"
shasum -a 256 \
  "$EXTERNAL/schema/typefacts-v1.schema.json" \
  "$EXTERNAL/schema/typefacts-codec-limits.json" \
  "$EXTERNAL"/benchmarks/phase1/*.cbor
go version
cargo +1.97 --version
```

Save outputs in the checked-in baseline JSON/report. The source manifest should record external path, blob ID, mode, byte size, mapped destination, and whether an adapter edit is permitted.

### 2. Build the external authority without touching checker `bin/`

```sh
cd "$EXTERNAL"
go build -trimpath -buildvcs=false \
  -ldflags "-X main.buildID=$PARITY_ID" \
  -o "$OLD_BIN" ./cmd/solid-typefacts

TYPEFACTS_BUILD_ID="$PARITY_ID" TYPEFACTS_TEST_BIN="$OLD_BIN" \
  cargo +1.97 test --workspace
go test -race ./...
go vet ./...
test -z "$(gofmt -l cmd internal shims)"
cargo +1.97 fmt --all -- --check
cargo +1.97 clippy --workspace --all-targets -- -D warnings
```

Also run `make benchmark-memory` and save the complete benchmark output before import.

### 3. Build and test the imported source

The exact local Go package path depends on the implemented layout; the intended command is:

```sh
cd "$CHECKER"
go build -trimpath -buildvcs=false \
  -ldflags "-X main.buildID=$PARITY_ID" \
  -o "$NEW_BIN" ./apps/solid-typefacts/cmd/solid-typefacts

TYPEFACTS_BUILD_ID="$PARITY_ID" TYPEFACTS_TEST_BIN="$NEW_BIN" \
  cargo +1.97 test --manifest-path rust/Cargo.toml -p typefacts
go test -race ./...
go vet ./...
test -z "$(gofmt -l apps/solid-typefacts shims)"
cargo +1.97 fmt --manifest-path rust/Cargo.toml --all -- --check
cargo +1.97 clippy --manifest-path rust/Cargo.toml -p typefacts --all-targets -- -D warnings
```

### 4. Cross-pair clients and producers

Run the external crate's full process suite once with each binary, then the imported crate's full process suite once with each binary:

```sh
TYPEFACTS_BUILD_ID="$PARITY_ID" TYPEFACTS_TEST_BIN="$OLD_BIN" \
  cargo +1.97 test --manifest-path "$EXTERNAL/Cargo.toml" -p typefacts --test session_process
TYPEFACTS_BUILD_ID="$PARITY_ID" TYPEFACTS_TEST_BIN="$NEW_BIN" \
  cargo +1.97 test --manifest-path "$EXTERNAL/Cargo.toml" -p typefacts --test session_process

TYPEFACTS_BUILD_ID="$PARITY_ID" TYPEFACTS_TEST_BIN="$OLD_BIN" \
  cargo +1.97 test --manifest-path "$CHECKER/rust/Cargo.toml" -p typefacts --test session_process
TYPEFACTS_BUILD_ID="$PARITY_ID" TYPEFACTS_TEST_BIN="$NEW_BIN" \
  cargo +1.97 test --manifest-path "$CHECKER/rust/Cargo.toml" -p typefacts --test session_process
```

Run the new transcript comparator with `OLD_BIN` and `NEW_BIN`; require handshake bytes and normalized semantic response bytes to match. Run the deliberate mismatched-build test too.

### 5. Checker lifecycle and findings

Build one fresh checker after the local path dependency is active, then use the same checker binary for all local-producer gates:

```sh
cd "$CHECKER"
SOLID_CHECKER_BUILD_ID="$PARITY_ID" TYPEFACTS_BUILD_ID="$PARITY_ID" \
  cargo +1.97 build --manifest-path rust/Cargo.toml \
  -p solid-facts-backend --bins

SOLID_TYPEFACTS_BIN="$NEW_BIN" TYPEFACTS_BUILD_ID="$PARITY_ID" \
  cargo +1.97 test --manifest-path rust/Cargo.toml \
  -p solid-facts-backend \
  --test transport_process --test sessions_process \
  --test reachability_parity_process --test owner_parity_process \
  --test cross_file_digest_process --test cross_file_callbacks_process \
  --test props_reactivity_process

SOLID_CHECKER_GATE_CACHE=0 \
SOLID_CHECKER_BIN="$CHECKER/rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$NEW_BIN" bun scripts/coverage.mjs

SOLID_CHECKER_GATE_CACHE=0 \
SOLID_CHECKER_BIN="$CHECKER/rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$NEW_BIN" bun scripts/ownership-gate.mjs \
  --require-retained --require-complete
```

Then run `make verify` with the ordinary local build path. Compare exact coverage snapshot/report hashes to the frozen baseline; counts alone are insufficient.

Also run the TypeScript oracle, obligation audit, contract process/conformance/corpus/differential gates because Type Facts supplies their proof premises:

```sh
make tsc-oracle
make obligation-audit
make contract-conformance
make contract-corpus
make contract-differential
```

### 6. Performance and memory A/B

Generate the same corpus once and keep the same release checker/session-bench binary for both producer sides.

```sh
bun benchmarks/generate-bench-corpus.mjs 5000 /tmp/bench-corpus-5k
cargo +1.97 build --release --manifest-path rust/Cargo.toml \
  -p solid-facts-backend --bin solid-checker-rust --bin solid-checker-session-bench

SOLID_CHECKER_MAX_RELATIVE_REGRESSION=1.10 \
bun benchmarks/compare-performance.mjs \
  --base-bench rust/target/release/solid-checker-session-bench \
  --base-typefacts "$OLD_BIN" \
  --head-bench rust/target/release/solid-checker-session-bench \
  --head-typefacts "$NEW_BIN" \
  --rounds 7
```

Run the external and imported `benchmark-memory` test/benchmark sets with `-count=10`, then compare with `benchstat`. For physical memory, alternate old/new for at least three runs each:

```sh
SOLID_CHECKER_BIN="$CHECKER/rust/target/release/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$OLD_BIN" \
SOLID_CHECKER_MEMORY_PROJECT=/tmp/bench-corpus-5k/tsconfig.json \
bun benchmarks/measure-retained-memory.mjs --vmmap-all --idle-secs=30 --max-physical-mib=1200

SOLID_CHECKER_BIN="$CHECKER/rust/target/release/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$NEW_BIN" \
SOLID_CHECKER_MEMORY_PROJECT=/tmp/bench-corpus-5k/tsconfig.json \
bun benchmarks/measure-retained-memory.mjs --vmmap-all --idle-secs=30 --max-physical-mib=1200
```

Save all JSON, not only the verdict. Repeat in alternating order to control thermal drift.

### 7. Packaging and clean-state proof

```sh
git status --short
make clean-verify
make verify
make package
```

Inspect the package inventory for the local `solid-typefacts` executable on every supported target. Search the repository for the old repository URL, revision extraction, `.typefacts` checkout, clone/fetch, and external-cache assumptions; only provenance/history documentation may retain the URL or commit.

Finally, require two clean CI runs and one release build before archiving the external repository. Archival is not part of the source-import commit.

## Required invariants for a no-protocol-delta import

1. Protocol remains 2; lifecycle schema remains 1; active table schema remains 17.
2. Schema, codec-limits, and all golden bytes remain unchanged.
3. The handshake map has exactly the same three fields and refuses unknown fields and every mismatched identity.
4. The operation enum and every request/response field/tag remain unchanged.
5. All deterministic semantic response bytes match after only numeric timing values are normalized.
6. No unknown field, enum tag, malformed transition, stale generation, foreign location, or contradictory fact becomes accepted.
7. Cancellation/failure never commits candidate state; restart replays exactly the newest overlay for each path.
8. Retained incremental answers equal fresh materialization at every adversarial generation.
9. The Rust client stays the sole checker-facing seam; Reactive IR learns no TypeScript-Go, CBOR, transport, or Go types.
10. Every producer source change invalidates the binary stamp and all producer-dependent gate caches.
11. The local producer and local client are built and switched atomically.
12. No new semantic fact, protocol field, schema version, dependency upgrade, performance refactor, or cleanup is allowed in the relocation commits.
13. Checker findings, classifications, spans, and uncertifiable results have no unexplained delta.
14. Old documents and ADRs retain their original numbers and historical claims.

## Exit recommendation

Accept Type Facts repatriation only after the transcript comparator exists, the timing normalization is explicit and narrow, the manifest-derived identity binds codec limits without changing the handshake shape, all four client/producer pairings pass, and the full checker/relative-resource gates are recorded. The existing test corpus is strong enough for the move; the missing work is orchestration and durable before/after evidence, not more semantic facts.
