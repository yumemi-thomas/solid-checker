# Type Facts repatriation: solid-checker integration audit

Audit date: 2026-08-27

Scope: read-only inventory of active and historical Type Facts producer/client
dependencies in `solid-checker`, plus the exact integration changes needed to
make the producer and Rust client locally owned without changing facts or the
wire protocol.

## Import authority and immediate correction

- The active Cargo dependency, Cargo lock, ignored producer stamp, and clean
  external checkout all agree on
  `yumemi-thomas/solid-ts-facts@92c53392388518d69ef27220729f5c061479deed`.
- That commit is a merge commit whose parents are `4e11b602...` and
  `cadb247b...`; importing the commit history, rather than copying its working
  tree, is necessary to retain both fact lines.
- `THIRD_PARTY_NOTICES.md` currently claims `19671a889b27...`, an intermediate
  merge that predates the primitive-literal and parameter-object facts. That
  notice is stale and must not become the local import identity.
- The same notice claims TypeScript-Go revision `2bd066d87f5b`, but the imported
  root module and all nine shim modules actually pin
  `v0.0.0-20260724234109-8d29e62f3585`. Record the exact pseudo-version. The
  shims are copied/adapted from tsgolint and now vendored; the current notice's
  statement that neither repository is copied is no longer true.
- The clean external checkout is at the correct commit. Its active schema hash
  is `sha256:9a217ca6aa3b147f84cd356df069259ecd548328ab0c48c83109832d1cbedeb9`;
  handshake protocol is 2.

## Required ownership layout and relocation rewrites

The planned layout is sound:

```text
go.mod / go.sum
apps/solid-typefacts/
shims/
rust/crates/typefacts/
schema/typefacts-*.json
docs/typefacts/
benchmarks/typefacts/
```

Use a history-preserving prefix import at the exact commit, then a distinct
relocation commit. The imported root scaffolding (`README`, `Makefile`, Cargo
workspace, CI/release workflows) must be either adapted into the owning local
file or deleted with a relocation ledger; it must not remain as dead build
instructions.

Required source-path changes after relocation:

1. Change the root Go module identity from
   `github.com/yumemi-thomas/solid-ts-facts` to
   `github.com/yumemi-thomas/solid-checker`.
2. Rewrite every Go self-import to
   `github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/...`.
3. Build the command as `./apps/solid-typefacts`, not
   `./cmd/solid-typefacts`.
4. Keep all nine `shims/*` module identities and root `replace` directives;
   they intentionally claim TypeScript-Go shim module paths. Do not rewrite
   those module names to solid-checker.
5. In `rust/crates/typefacts`, `CARGO_MANIFEST_DIR/../..` no longer reaches the
   repository root. Use `../../..` in tests and benchmarks, then address test
   data under `apps/solid-typefacts/internal/typefacts/testdata`.
6. Rewrite Rust source-relative assets:
   - `src/v3.rs`: schema at `../../../../schema/typefacts-v1.schema.json` and
     response golden at
     `../../../../benchmarks/typefacts/phase1/typefacts-v3-response-golden.cbor`;
   - `src/session.rs`: repository-root-relative
     `benchmarks/typefacts/phase1/typefacts-v5-transition-golden.cbor`;
   - `tests/typefacts_v3_codec_golden.rs`: `benchmarks/typefacts/phase1`;
   - process tests/bench fallback producer: `./apps/solid-typefacts`.
7. Preserve the Type Facts copyright as `apps/solid-typefacts/LICENSE` (or an
   equivalent dedicated license file). The root MIT license has a different
   copyright line and is not a byte-equivalent substitute.
8. Preserve ADR names as imported. There are deliberately two files numbered
   0021; do not renumber them into the checker ADR namespace.

The WASM prototype needs special treatment. Its Go command imports private
producer packages. A command built from `packages/wasm` cannot import
`apps/solid-typefacts/internal` under Go's `internal` rule. Remove its
`.typefacts` git checkout/pin assertion, update imports, and create its temporary
command directory underneath `apps/solid-typefacts` before building and
removing it. Alternatively move the prototype command source under that app.

## Rust workspace migration

Required atomic Cargo changes:

- Add `crates/typefacts` to `rust/Cargo.toml` workspace members.
- Replace the git workspace dependency with
  `typefacts = { path = "crates/typefacts" }`.
- Add the imported client's workspace dependencies (`ciborium = "0.2.2"` and
  `serde_bytes = "0.11.15"`); existing `serde`, `sha2`, and `thiserror`
  requirements already cover it.
- The imported crate cannot retain `version.workspace = true`, because the
  checker root declares no workspace version. Preserve its exact
  `version = "0.10.0"`; use the monorepo's `publish = false` policy.
- Regenerate `rust/Cargo.lock`. The `typefacts` package must have no git source
  and the external repository URL must disappear from the active lock.
- Keep the package name and public Rust API unchanged. All current consumers
  (`solid-facts`, `solid-reactive-ir`, `solid-facts-backend`, and
  `solid-checker-wasm`) can continue using `typefacts.workspace = true`.

Do not split this into a local producer with git client or local client with
external producer. The path dependency, command source, schema assets, build
script, and lockfile must switch together.

## Build identity, ignored binary, and cache requirements

`scripts/build-typefacts.sh` currently parses a Cargo `rev`, clones/fetches a
repository into `.typefacts`, checks out detached HEAD, and stamps
`revision=<sha> build-id=<id>`. All of that active external-repository behavior
must go.

The local builder should:

1. Compute a deterministic manifest over on-disk contents of:
   `apps/solid-typefacts`, `rust/crates/typefacts`, `shims`, both Type Facts
   schemas, root `go.mod`/`go.sum`, relevant build scripts, and the build ID.
   Include paths as well as bytes so rename/add/delete changes the identity.
2. Include the selected Go toolchain/target identity (`go` directive or action
   version, actual `go env GOVERSION`, GOOS, GOARCH, and CGO mode as applicable)
   in the build/stamp/cache identity.
3. Build `./apps/solid-typefacts` with the same `-X main.buildID=...` value the
   Rust client receives through `TYPEFACTS_BUILD_ID`.
4. Write `bin/solid-typefacts.buildinfo` with at least source digest, build ID,
   toolchain/target identity, and preferably output SHA-256. Verify the output
   hash on a skip to close the current hand-replaced-binary hole.
5. Continue supporting `TYPEFACTS_REBUILD=1`; delete
   `SOLID_TYPEFACTS_CHECKOUT` and revision parsing.

`scripts/verify-delta.mjs` currently parses the deleted Cargo revision and its
tests know only `revision=...`. Replace `producerStampDrift` with a recomputed
local source-identity comparison, update its tests and wording, and retain the
fail-closed behavior for absent, malformed, or mismatched stamps. The universal
schema check must include both Type Facts JSON schemas, not only
`solid-reactivity.schema.json`.

Gate-result caches already hash the producer binary and `.buildinfo`; keeping
both in `coverage`, `ownership`, `tsc-oracle`, and contract-corpus inputs is
correct. The new source-digest stamp makes local source changes invalidate the
binary and therefore every producer-dependent gate. Update comments that call
the stamp a revision.

Keep `/.typefacts/` in `.gitignore` as protection for existing user checkouts,
but remove `.typefacts` from `make clean`. This workspace already contains a
clean ignored checkout at the import commit; deleting it is unnecessary and
destructive. It is a retired compatibility artifact, not an active input.

## Make and verification integration

Keep `SOLID_TYPEFACTS_BIN` as the explicit process boundary and packaged binary
override. Colocation does not justify in-process Go or a PATH search.

Required root targets and environment changes:

- `build-typefacts`: local source builder, same build ID as the client.
- `test-rust`: also set `TYPEFACTS_TEST_BIN=$PWD/bin/solid-typefacts`; the
  imported client process suite uses that variable and otherwise attempts its
  own fallback Go build.
- Add named Type Facts Go checks: `gofmt` over `apps/solid-typefacts` and
  `shims`, `go vet ./...`, and `go test -race ./...`.
- Add those checks to `scripts/verify.sh` and to the main CI job. Rust fmt,
  clippy, and workspace tests will automatically include the local client once
  it is a workspace member.
- Preserve the imported targeted memory gate and 5k retained-session gate,
  adapted to local paths. They may remain opt-in performance gates, but cannot
  be discarded.
- Update `verify-delta` path ownership. A change under
  `apps/solid-typefacts`, `rust/crates/typefacts`, `shims`, Type Facts schemas,
  `go.mod`, or `go.sum` must at minimum rebuild the producer and run Type Facts
  Go/client/protocol checks. Because Type Facts can change every finding, a
  semantic producer/client change should escalate to coverage and ownership or
  full verify.
- Ensure `sh -n scripts/*.sh` and `jq empty schema/*.json` remain gates.

The current checker-side protocol/process coverage must remain armed:

- `transport_process`: startup build mismatch rejection and retained open /
  sources / analyze / update path;
- `sessions_process`: pipeline ordering, recovery/replay, cancellation,
  crash-before-update/analyze, stale generations, module graph, retained facts;
- all other fixture-driven process tests guarded by
  `SOLID_TYPEFACTS_BIN`, including cross-file callbacks/digests,
  reachability/owner parity, props reactivity, contracts, dialects, and JSON
  imports;
- `harness_process` canary proving the environment is set.

## CI and release integration

`.github/actions/typefacts/action.yml` is an external-revision cache today. It
must key on the local manifest digest + build ID + runner OS/arch + declared Go
toolchain, restore the binary and stamp, install Go using root `go.mod` on a
miss, invoke the local builder, and verify the executable/stamp. All workflows
already set up Bun before the composite action, so a repository-owned Bun
identity helper is viable on every current caller.

Active callers that must remain valid:

- `.github/workflows/ci.yml` engine and four native-package targets;
- `contract-corpus.yml`;
- `corpus.yml`;
- ecosystem sentinel and full-corpus jobs;
- performance head and merge-base builds;
- npm release matrix.

Add Type Facts-owned paths to the `contract-corpus.yml` and
`ecosystem-benchmark.yml` path filters. Otherwise a producer semantic change
can avoid those workflows entirely.

The performance merge-base deserves an explicit transition rule: during the
first repatriation run, the merge-base predates local Type Facts. Either allow
the action to build a historical base through that base's old builder only for
the comparison, or reset the relative performance baseline with a documented
external-vs-local parity measurement. Do not make the new local-head builder
pretend old commits contain local source. Once main contains the import, normal
base/head builds are both local.

Release packaging itself is already correctly binary-oriented:
`package-rust.mjs`, the npm binding assembly, launcher sibling discovery, and
native manifest all ship/locate `solid-typefacts` by file and hash. They do not
need architectural changes. Update the launcher's stale comment saying Go no
longer lives in this repository. Preserve cross-platform CI and release builds.

## Protocol invariants and a documentation correction

The active startup handshake contains exactly:

```text
protocol + schemaHash + buildId
```

Codec limits are checked against `schema/typefacts-codec-limits.json` by Go
tests and enforced independently by codecs, but are **not a handshake field**.
Bootstrap/monorepo prose that says the handshake compares codec limits is
currently inaccurate. For parity, preserve the three-field handshake and
correct the prose. Adding a codec-limit digest to the handshake would be a
later coordinated protocol improvement, not part of source repatriation.

Preserve these imported proof surfaces:

- deterministic Go/Rust CBOR goldens and schema digest tests;
- unknown/duplicate-field and codec-bound rejection;
- lifecycle operation, compact demand, packed transition, and transition arena
  tests;
- cancellation, restart, transaction rollback, stale-symbol, reference-change,
  source-digest, module-graph, and retained-memory adversarial tests;
- all 181 imported Go tests/benchmarks visible at the selected commit;
- Rust public API, codec golden, retained-table, shared-arena, session process,
  and benchmark coverage.

## Parity proof required before any new facts

Record both machine JSON and a human report. Minimum proof:

1. External checkout is clean and exactly `92c533...`; record its tree and
   schema/golden hashes.
2. Run the external repository's Go and Rust suites with one build ID.
3. Run the relocated Go and local Rust workspace suites with the same build ID.
4. Feed identical lifecycle request transcripts to external and local producer
   binaries. Compare handshake bytes and every response byte. If a relocation
   changes only an identity field, compare decoded semantics separately and
   name the exact identity-only difference.
5. Compare all imported schema and golden bytes before intentional path-only
   edits; schema SHA and protocol number must remain unchanged.
6. Run checker adapter/process tests against the local binary.
7. Run cache-disabled fixture coverage and ownership; finding snapshots must be
   unchanged.
8. Run cancellation, crash/restart/replay, stale generation, module-graph, and
   retained-session adversarial tests.
9. Run memory/performance baselines against external and local binaries; source
   movement alone may not silently reset budgets.
10. Run native packaging smoke tests so the sibling producer is actually used
    with `SOLID_TYPEFACTS_BIN` unset.
11. Run full `make verify`.

Do not add resolved-invocation or other fact improvements until this parity
record is green. Source ownership and fact semantics need separate commits and
reports.

## Documentation/reference classification

Active references that must change to local ownership include:

- `AGENTS.md`, `CONTRIBUTING.md`, `rust/README.md`;
- root `Makefile`, builder, verify-delta, verify script, composite CI action;
- `docs/typefacts.md`, `docs/tsgolint-extraction.md`, `docs/monorepo.md`,
  compiler/Type Facts bootstrap completion status, docs index;
- `THIRD_PARTY_NOTICES.md` (import baseline and local license locations;
  TypeScript-Go/shim resolution now comes from root Go files, and the vendored
  tsgolint-derived shims retain `shims/LICENSE`);
- imported README/Makefile/docs with old repository-relative paths;
- WASM prototype checkout/import assumptions;
- comments in CLI launcher and ecosystem instructions describing a checked-in
  or externally pinned producer.

Historical references should remain historical, not be mechanically rewritten:

- Phase 0 baseline JSON/Markdown and its `revision=92c533...` buildinfo;
- ecosystem reports naming the exact producer used for an old measurement;
- precision-backlog citations to individual solid-ts-facts commits/ADRs;
- baseline/planning/subagent reports explaining why `92c533...` was selected.

`scripts/package-contract-v2-phase0.mjs` reconstructs a historical freeze from
live Cargo pins and will no longer parse a path dependency. The compiler
bootstrap already made that reconstruction conceptually stale. Do not rewrite
the frozen Phase 0 artifact. Either mark the script as replay-only at the
baseline commit or teach current reporting to record a separate local source
digest without mutating the historical baseline.

## Completion conditions

The repatriation is complete only when:

- no active Cargo/build/CI path fetches or parses an external Type Facts pin;
- producer and client are local in the same commit and no mixed state exists;
- the local source identity invalidates ignored binary and gate caches;
- Go, Rust, protocol, lifecycle, memory, checker, package, and full verification
  gates are green;
- schema hash, handshake protocol, decoded facts, and checker findings are
  unchanged;
- active docs describe local ownership, while historical provenance remains;
- the external repository can be made read-only after two clean CI runs and one
  release build, with no PR needed for future semantic-fact work.

## Active integration file inventory

This is the concrete review checklist. Files that merely use the public
`typefacts` Rust API are not all source-ownership edits, but every active
producer/client boundary below was inspected.

| Concern | Active files |
| --- | --- |
| Cargo client identity | `rust/Cargo.toml`, `rust/Cargo.lock`, `rust/crates/{solid-facts,solid-reactive-ir,solid-facts-backend,solid-checker-wasm}/Cargo.toml` |
| Producer build and orchestration | `scripts/build-typefacts.sh`, root `Makefile`, `scripts/verify.sh`, `scripts/verify-delta.mjs`, `scripts/verify-delta.test.mjs` |
| CI binary cache/build | `.github/actions/typefacts/action.yml` |
| CI callers | `.github/workflows/{ci,contract-corpus,corpus,ecosystem-benchmark,performance,publish-npm}.yml` |
| Gate cache binary/stamp inputs | `scripts/{coverage,ownership-gate,tsc-oracle-gate,contract-corpus}.mjs`, `scripts/lib/gate-cache.mjs` and its tests |
| Other explicit producer consumers | `scripts/{contract-differential,contract-closure-record.test,contract-generation.test,contract-review.test,contract-verify.test,obligation-audit,tsc-oracle}.mjs`, ecosystem runner/verifier, and benchmark scripts |
| Package/release layout | `scripts/package-rust.mjs`, `scripts/assemble-npm-package.mjs` and test, `packages/cli/bin/launcher.mjs`, `packages/cli/README.md` |
| WASM boundary/prototype | `packages/wasm/prototypes/self-contained.mjs`, relocated reactor command, `packages/wasm/README.md`, WASM tests |
| Checker process boundary | `rust/crates/solid-facts-backend/src/{main,daemon,wire,diagnostics,demand_plan,cache,lib}.rs` plus benchmark binaries |
| Checker process tests | backend `tests/{harness,transport,sessions,contracts,dialects,cross_file_callbacks,cross_file_digest,reachability_parity,owner_parity,props_reactivity,json_import_contract,diagnostics}_process.rs` and shared support |
| Rust semantic consumers | `rust/crates/solid-facts/src/{core,project,resolution}.rs` and the `solid-reactive-ir` modules importing `typefacts` facts |
| Ignore/cleanup | `.gitignore`, root `Makefile clean`, `AGENTS.md` caveats |
| Active ownership docs | `CONTRIBUTING.md`, `rust/README.md`, `docs/{typefacts,tsgolint-extraction,monorepo}.md`, `docs/README.md`, package-contract-v2 architecture/bootstrap/plan docs, `THIRD_PARTY_NOTICES.md` |
| Historical identity only | Phase 0 baseline artifacts/script, ecosystem reports, precision-backlog commit citations, Phase 0 subagent report |
