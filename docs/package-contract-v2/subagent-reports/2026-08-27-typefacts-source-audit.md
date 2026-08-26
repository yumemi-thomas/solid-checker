# Phase 0A Type Facts source audit

## Verdict

`/Users/thomas/Documents/Github/solid-ts-facts` is a suitable behavior-preserving import source at the checker pin. Import the commit object, not the working directory, and use a two-commit sequence: an unsquashed history import under a temporary prefix, followed by a mechanical relocation into the approved monorepo layout. Protocol or semantic improvements must remain later commits.

## Frozen source identity

- Repository: `https://github.com/yumemi-thomas/solid-ts-facts.git`
- Branch: `main`; `HEAD`, `origin/main`, and `origin/HEAD` all resolve to `92c53392388518d69ef27220729f5c061479deed`.
- Commit: merge `92c5339`, parents `4e11b602034fc105c284a1efc1074569c04d9bdb` and `cadb247b4d7a1f27060ab948d50acd3707fe596b`, dated 2026-08-26T22:46:50+09:00.
- Tree: `c2da8d0ceb90d9090ed588623a4eec56e5e3973c`.
- Worktree: clean by `git status --porcelain=v1 --untracked-files=all` before the audit. There are ignored build products, so the import must still address the commit, never filesystem contents.
- History: 98 commits, 205 tracked files, 1,570,264 tracked bytes. The head is 73 commits after tag `v0.1.3`; the release tags are `v0.1.0` through `v0.1.3`, and no tag points at the pinned head.
- No submodules or Git LFS objects; tracked files use ordinary `100644` mode.
- Source repository has no `AGENTS.md`. Its canonical terminology is in `CONTEXT.md`.

## Baseline behavior and pins

- Go toolchain: `go1.26.5 darwin/arm64`; `go.mod` requires Go 1.26.
- Rust toolchain: Cargo 1.97.1; the client crate is `typefacts` 0.10.0, edition 2024, Rust 1.97.
- TypeScript-Go: all nine audited shim modules pin exactly `v0.0.0-20260724234109-8d29e62f3585`; `pins_test.go`, the import allowlist, and the `go:linkname` signature audit enforce the seam.
- Lifecycle schema: active public schema 1; startup handshake protocol 2; schema SHA-256 `9a217ca6aa3b147f84cd356df069259ecd548328ab0c48c83109832d1cbedeb9`; active packed Wire table schema 17.
- Codec-limits SHA-256: `3f511a4bf87d91fcffa021a21942458ce413771153e17da49c383d9bbd4beff0`.
- The source handshake currently contains protocol, schema hash, and build ID. It does **not** carry a separate codec-limits digest. Preserve this behavior during the parity import. The planned local source-manifest/build identity should bind the codec-limits file; an explicit fourth handshake identity would be a later protocol change, not part of relocation.
- All source-owned gates passed from a clean clone of the exact commit: `gofmt`, `go vet`, Rust format, workspace Clippy, Go race tests in four packages, 33 Rust unit tests, 1 public-API test, 26 producer/client process tests, 3 CBOR golden tests, and doc tests. An initial run in the read-only source checkout failed only when tests tried to create test binaries there; rerunning the exact commit in `/tmp` passed completely.

Frozen artifact hashes:

| Artifact | SHA-256 |
| --- | --- |
| `schema/typefacts-v1.schema.json` | `9a217ca6aa3b147f84cd356df069259ecd548328ab0c48c83109832d1cbedeb9` |
| `schema/typefacts-codec-limits.json` | `3f511a4bf87d91fcffa021a21942458ce413771153e17da49c383d9bbd4beff0` |
| module-graph request golden | `ea908beca2c149b43a049583d748c31f1be26d8bdbb7b279bb5503be779ec527` |
| module-graph response golden | `0fa3434f951e9a37705c32f14b480fe523c7304571f71cc60480295b2d4a3437` |
| v3 request golden | `bfda90e8cc17a0129dce70e990fa8272fead6bb0272681960afdd16cec12a578` |
| v3 response golden | `8494ad7a589611e139af1f89a488789ea58cede9a69e975ba9ae0a56a8c2ff62` |
| v5 transition golden | `b91272212c1cbff9743fdccee40acfe7bac1c4aaa432ad7bed6230e36a53fbcf` |

## History-preserving import procedure

1. Add/fetch the external repository and verify the fetched object is exactly `92c53392388518d69ef27220729f5c061479deed` with tree `c2da8d0c...`.
2. On a clean checker branch, use an unsquashed subtree import under a temporary prefix such as `.typefacts-import`, pinned by commit hash, for example `git subtree add --prefix=.typefacts-import <local-or-fetched-repository> 92c53392388518d69ef27220729f5c061479deed`. Do not use `--squash`.
3. Commit no adaptations in the import commit. Its prefixed tree must match the frozen source tree byte-for-byte.
4. In the next commit, use `git mv` for the owned paths below. Make only path/module-reference adaptations necessary to compile in the new layout.
5. Integrate workspace/build/CI scripts and switch producer plus client atomically in the following parity commit. Do not leave an external-producer/local-client or local-producer/git-client intermediate commit advertised as usable.
6. Remove `.typefacts-import` only after every imported file has a destination or an explicit superseded ruling.

The repositories do not currently share commit objects in the checker object database, so a normal merge is not a useful relocation mechanism. Unsquashed subtree import retains the complete source ancestry through its merge parent while giving the imported snapshot one collision-free prefix.

## Proposed relocation map

| Imported source | Monorepo owner | Notes |
| --- | --- | --- |
| `cmd/solid-typefacts/*` | `apps/solid-typefacts/*` | Put `main.go`, its tests, arena files, and lifecycle benchmark directly in the app root. This is the layout previously established by checker commit `93e0a3c6` and preserves Go `internal` visibility. |
| `internal/typefacts/**` | `apps/solid-typefacts/internal/typefacts/**` | Producer model, retained sessions, TypeScript-Go adapter, adversarial tests, and testdata. |
| `internal/wirecbor/**` | `apps/solid-typefacts/internal/wirecbor/**` | Private deterministic-CBOR codec. |
| `shims/**` | `shims/**` | Preserve all nine modules, each `go.sum`, shim source, and `shims/LICENSE` byte-for-byte apart from required repository-relative audit paths. |
| `crates/typefacts/**` | `rust/crates/typefacts/**` | Rust process/session client, codec, retained arena, benches, and process/golden tests. |
| `schema/typefacts-*.json` | `schema/typefacts-*.json` | Preserve bytes and hashes during parity. |
| `benchmarks/phase1/**` | `benchmarks/typefacts/phase1/**` | Retain the historical `phase1` grouping under the new Type Facts owner; update both Go and Rust golden paths. |
| `docs/adr/**` | `docs/typefacts/adr/**` | Preserve numbering exactly, including the two historical ADRs numbered `0021`; do not renumber into checker ADRs. |
| `docs/compiler-semantic-facts.md` | `docs/typefacts/compiler-semantic-facts.md` | Producer/compiler semantic interface. |
| `docs/migration-solid-checker.md` | `docs/typefacts/migration-solid-checker.md` | Keep as migration evidence; reconcile live links from checker docs. |
| performance/design markdown under `docs/` | `docs/typefacts/**` | Includes warm incremental, memory, and Go/Rust exploration reports. |
| `docs/solid-checker-reactive-ir-performance.patch` | `docs/typefacts/archive/solid-checker-reactive-ir-performance.patch` | Historical evidence, not an active patch to apply. |
| `CONTEXT.md` | `docs/typefacts/glossary.md` plus root `CONTEXT.md` reconciliation | Keep the original domain text under the Type Facts owner and merge its canonical terms into the root glossary without creating competing spellings. |
| `README.md` | `docs/typefacts/README.md` | Update paths/module name only after the byte-preserving import commit. |
| `LICENSE` | `docs/typefacts/LICENSE` | Preserve the exact MIT notice for Type Facts contributors. Root checker remains MIT; also record the imported component in `THIRD_PARTY_NOTICES.md`. |
| `go.mod`, `go.sum` | root `go.mod`, `go.sum` | Checker currently has neither, so there is no collision. Change module identity to `github.com/yumemi-thomas/solid-checker` and rewrite private imports to `.../apps/solid-typefacts/internal/...` in the relocation/build commit. |
| root `Cargo.toml`, `Cargo.lock` | merge into `rust/Cargo.toml`, regenerate `rust/Cargo.lock` | Add `crates/typefacts` to the existing workspace and fold its five dependencies into workspace dependencies. Do not retain a second Rust workspace root. |
| source `Makefile` targets | existing root `Makefile` and scripts | Merge test and benchmark obligations; do not replace checker targets. |
| source `.github/workflows/ci.yml` | existing checker CI | Transcribe Go race/vet/fmt and Rust client/process coverage into checker jobs. |
| source `.github/workflows/release.yml` | checker release/package pipeline | Preserve the five-platform producer build matrix as a packaging obligation; do not activate a second independent release workflow. |
| source `.gitignore` | root `.gitignore` | Merge only relevant Go/Rust producer products. |

## Required path-only adaptations

- Rewrite all `github.com/yumemi-thomas/solid-ts-facts/internal/...` imports to `github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/...`.
- Change build/test command paths from `./cmd/solid-typefacts` to `./apps/solid-typefacts`.
- The relocated Go pin and linkname audits move two directory levels deeper: their `../../go.mod` and `../../shims/...` walks become root-aware equivalents (the earlier checker relocation used `../../../../`).
- Update Rust `include_bytes!` and test/benchmark paths for schemas, Type Facts goldens, Go testdata, and the producer command. These occur in `v3.rs`, `session.rs`, `retained_session.rs`, `session_process.rs`, and `typefacts_v3_codec_golden.rs`.
- Update the Go golden readers whose repository-relative paths currently name `schema/` or `benchmarks/phase1/`.
- Keep the TypeScript-Go pin at `8d29e62f3585`; current checker `docs/typefacts.md` still documents the older `2bd066d87f5b` revision and must be corrected or superseded by the imported documentation.
- Preserve the schema and golden bytes. Path relocation must not regenerate them.

## License and provenance obligations

- Root Type Facts license: MIT, copyright 2026 Type Facts contributors.
- Shim license: MIT, with VoidZero/typescript-eslint attribution. It must remain alongside `shims/`.
- The source module's dependency licenses are not copied source licenses; the checker notice/package audit should continue to account for compiled dependencies separately.
- Preserve the external commit ID, tree ID, imported merge parent, TypeScript-Go pseudo-version, schema hash, codec-limits hash, and golden hashes in the final conformance report.

## Risks to block during implementation

1. **Mixed producer/client state:** the path dependency and local build must switch together. A transient import commit may be non-building, but no product path may use only half of it.
2. **Build identity false parity:** relocation changes source location and should replace revision stamps with a deterministic source-manifest digest. Compare decoded semantic responses separately if the handshake build ID necessarily changes.
3. **Hidden path drift:** several tests compile producers dynamically or walk upward to `go.mod`, `shims`, schemas, goldens, and testdata. A shallow `go test` is insufficient; the Rust process suite is the relocation canary.
4. **Codec-limit identity:** codec limits are tested but are not an independent handshake field today. Bind them into the local manifest digest during repatriation without changing wire protocol; decide any explicit handshake field only after parity.
5. **Cache invalidation:** the new manifest must cover `apps/solid-typefacts`, `rust/crates/typefacts`, `shims`, both Type Facts schemas, root Go module files, Go and Rust toolchain identity, TypeScript-Go pin, and build ID. Gate-cache keys must consume the same identity.
6. **Documentation collision:** root `docs/typefacts.md` describes the external location and an obsolete compiler pin. It should become an overview pointing into `docs/typefacts/`, not remain competing authority.
7. **Release regression:** external CI builds five producer targets (Linux x64/arm64, macOS x64/arm64, Windows x64). Checker packaging must retain equivalent coverage before the external repository is retired.
8. **Premature cleanup:** do not archive the external repository until two clean monorepo CI runs and one release build, as specified by the bootstrap plan.

## Minimum parity evidence after relocation

- Byte-for-byte request/response transcript comparison against an external binary built from `92c5339`, with the same build ID.
- Exact schema/golden hash comparison and deterministic-CBOR round trips.
- Imported `make test` obligations: Go race tests, vet, gofmt; Rust format, Clippy, unit, public API, process/lifecycle, crash/restart/cancellation/stale-generation, and golden tests.
- Memory and retained-session gates, including `benchmark-memory`; record the 5k checker corpus measurement separately because it depends on generated corpus availability.
- Checker process, coverage, ownership, and full `make verify` with unchanged findings.
- Local build script test proving it performs no clone/fetch/checkout and invalidates on every manifest-owned input.
- CI/package audit proving no active reference still treats `solid-ts-facts` as a git dependency or external checkout.
