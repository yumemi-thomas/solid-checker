# RC.13 compiler-facts candidate

This directory preserves the local compiler candidate based on published Solid
`2.0.0-rc.13`. Production pins are unchanged. See the
[rebase report](../../../docs/package-contract-v2/phase22/2026-10-03-rc13-compiler-facts-rebase.md).

## Contents

- `semantic-facts.patch`: complete compiler-only delta against official
  `5efaf260becb32293f2bcb4d32f8be72be6de674`, including focused tests and the
  independently generated output baseline.
- `candidate.bundle`: Git objects preserving exact candidate implementation
  and distribution commits. This prevents build-tree cleanup losing the fork.
- `candidate-evidence.json`: identities, registry metadata, successful check
  hashes and output comparison summaries. `authority` and `certification` are
  false; this is development evidence.
- `probe/` and `probe-upstream/`: direct Rust compile drivers; only the candidate
  is built with `--features trace`.
- `adapter/`: isolated copy of the production projection with candidate pins.
  Its dependency is the candidate checkout, not the production Git pin.
- `compare.mjs`: exact source/output and trace-neutrality comparisons.
- `compare.test.mjs`: rejects stale provenance/digests, output drift, incomplete
  results and unsupported fact admission.
- `capture-release.mjs` / `compare-installed.mjs`: isolated published-artifact
  collection with integrity verification, and native artifact comparison.
- `verify-candidate.mjs`: independently checks Git scope and identity commit,
  bundle prerequisites, patch bytes, successful artifacts and input hashes.

## Checkout locations

The active isolated checkouts are under the main repository:

```text
rust/target/compiler-rc13-upstream  official RC.13
rust/target/compiler-rc13-rebase    codex/compiler-facts-rc13
rust/target/compiler-rc13-build     separate Cargo target directory
```

To reconstruct them after cleanup, clone official Solid into each empty
checkout directory. Check out upstream commit `5efaf260...` in the first.
In the second, fetch the saved bundle and check out its exact distribution:

```sh
git fetch /absolute/path/to/benchmarks/compiler-facts/rc13/candidate.bundle refs/heads/codex/compiler-facts-rc13
git checkout --detach 3ad4bbec37ae30f325a803cdb4271a71c86a2a2d
```

The bundle requires the published upstream history, which the official clone
provides. A standalone patch application reproduces bytes but does not preserve
commit identity. Use the bundle for identity validation.

Both nested checkouts need an untracked root `Cargo.toml` so Cargo does not use
the enclosing checker's workspace:

```toml
[workspace]
resolver = "2"
members = ["packages/compiler"]
```

## Local whole-checker implementation

`checker-automatic-integration.patch` is the complete current overlay against
`c6d973abb2910340ed72322a95fd055cf13d7860`. It adds native automatic source
models, executed-read warnings, observer-query hints and the pending callback
precision fix. Use `render-automatic-integration.mjs` in the restoration command
below to reproduce this version. It replaces the earlier overlay in a clean
checkout; do not apply both overlays. The final measurements and validation
scope are in the [automatic feedback report](../../../docs/package-contract-v2/phase22/2026-10-03-native-automatic-feedback-implementation.md).

`checker-integration.patch` preserves the local checker overlay against
`c6d973abb2910340ed72322a95fd055cf13d7860`. It includes the RC.13 compiler
identities, development-feedback command, precision fix and scoped fixture
updates. The compiler URI is a parameter rather than a fixed machine path.
Keep this as a local development overlay until a durable upstream dependency
location is chosen. The main checkout's production compiler pin is unchanged.

Restore the compiler from the bundle as above. In a clean checker checkout at
the stated base, render and inspect the overlay, then apply it:

```sh
node /absolute/path/rc13/render-automatic-integration.mjs /absolute/path/compiler-candidate > /tmp/checker-rc13.patch
git apply --check /tmp/checker-rc13.patch
git apply /tmp/checker-rc13.patch
make build-checker-debug
make verify
```

The renderer validates the exact compiler HEAD and refuses tracked compiler
changes. It prints a patch and changes no checkout. Existing uncommitted
checker edits need overlap review; applying this complete overlay twice is
not supported. Use the normal build targets so certification pins are present.
The local producer must be built/reused through `make build-typefacts`; this
overlay introduces no external producer/client.

Run the new command from the active main checkout with the tested local binary:

```sh
SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/local-rc13-checker/rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
node packages/cli/bin/solid-checker.mjs feedback --project /absolute/path/app/tsconfig.json
```

`checker-integration-evidence.json` retains verification and the eight-application
comparison. `check-pending-callback-typings.mjs` checks the new source against
published declarations; `check-html-refusal.mjs` compares malformed markup and
four successful compiler cases against the installed published RC.13 artifact.
The compiler upgrade does not upgrade the audited runtime, which remains RC.9.
Recorded browser inputs need a new execution if their local files are removed.
`checker-integration.patch` and its renderer preserve the earlier implementation
and evidence. `checker-automatic-integration-evidence.json` records the current
overlay digest and its independent application check against the stated base.

## Compiler-only reproduction

Run from the checker root. Cargo commands must run sequentially. These use
the separate target tree and do not replace the production checker binary.

```sh
cargo +1.97 test --offline --manifest-path rust/target/compiler-rc13-rebase/packages/compiler/Cargo.toml --no-default-features --target-dir rust/target/compiler-rc13-build
cargo +1.97 test --offline --manifest-path rust/target/compiler-rc13-rebase/packages/compiler/Cargo.toml --no-default-features --features tsrx --target-dir rust/target/compiler-rc13-build
cargo +1.97 test --offline --manifest-path rust/target/compiler-rc13-rebase/packages/compiler/Cargo.toml --target-dir rust/target/compiler-rc13-build
cargo +1.97 test --offline --manifest-path benchmarks/compiler-facts/rc13/adapter/Cargo.toml --target-dir rust/target/compiler-rc13-build
cargo +1.97 build --offline --manifest-path benchmarks/compiler-facts/rc13/probe-upstream/Cargo.toml --target-dir rust/target/compiler-rc13-build
cargo +1.97 build --offline --manifest-path benchmarks/compiler-facts/rc13/probe/Cargo.toml --features trace --target-dir rust/target/compiler-rc13-build
node --test benchmarks/compiler-facts/rc13/compare.test.mjs
node benchmarks/compiler-facts/rc13/compare.mjs rust/target/compiler-rc13-new-comparison
```

The comparator requires a new output directory and writes raw inputs and both
compiler outcomes before checking them. It decodes trusted upstream corpus
literals with the installed TypeScript parser; it does not execute corpus data.

Published-artifact collection requires network access and intentionally refuses
if the registry's `next` version has changed since this preparation:

```sh
node benchmarks/compiler-facts/rc13/capture-release.mjs rust/target/compiler-rc13-new-release
node benchmarks/compiler-facts/rc13/compare-installed.mjs rust/target/compiler-rc13-new-release rust/target/compiler-rc13-new-comparison rust/target/compiler-rc13-new-installed-comparison
```

For historical reproduction, retain the recorded RC.13 tarballs or collect the
exact recorded version and integrity explicitly; do not substitute the new tag.
The installed comparison currently requires darwin-arm64.

The completed JavaScript run rebuilt the native library and Babel plugin from
candidate source, using already installed local dependencies. Vitest used
`--configLoader native --no-cache` to keep reused dependency trees read-only.
The native loader used the candidate's local `compiler.node`, with no forced
WASI or external native override. Exact log hashes are in the evidence summary.

`verify-candidate.mjs` seals the completed run at its recorded artifact paths.
Changing those paths for a fresh run requires reviewing the new run's inputs
and updating the verifier; it must not silently label new artifacts historical.

Full checker verification, production pin migration, runtime-package alignment
and broader platform validation remain adoption work described in the report.
