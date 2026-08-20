#!/bin/sh
set -eu

rust_manifest=rust/Cargo.toml

cargo +1.97 fmt --manifest-path "$rust_manifest" --all -- --check
cargo +1.97 clippy --manifest-path "$rust_manifest" --workspace --all-targets
cargo +1.97 check --manifest-path "$rust_manifest" -p solid-facts-backend \
  --all-targets --no-default-features --features dialect-v1
cargo +1.97 check --manifest-path "$rust_manifest" -p solid-facts-backend \
  --all-targets --no-default-features --features dialect-v2
cargo +1.97 check --manifest-path "$rust_manifest" -p solid-checker-wasm \
  --all-targets --no-default-features --features dialect-v1
cargo +1.97 check --manifest-path "$rust_manifest" -p solid-checker-wasm \
  --all-targets --no-default-features --features dialect-v2

scripts/build-typefacts.sh
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  cargo +1.97 test --manifest-path "$rust_manifest" --workspace

cargo +1.97 build --manifest-path "$rust_manifest" --workspace
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/coverage.mjs
# The product-owned corpus carries exact checker expectations and per-finding
# TypeScript ownership for every retained former parity case.
node scripts/tsc-oracle.mjs provision --dialect all
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/ownership-gate.mjs \
  --require-retained

cargo +1.97 build --release --manifest-path "$rust_manifest" \
  -p solid-facts-backend --bin solid-checker-session-bench
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  node benchmarks/verify-performance.mjs

npm ci --ignore-scripts --prefix packages/cli
npm test --prefix packages/cli

# AGENTS.md's absolute rule, as a gate: no rule's positive case may also be a
# `tsc` error against the real published Solid typings. Provisioning installs
# the audited package versions and verifies them, so a drifted install fails
# here rather than changing the answer silently. The gate runs the checker over
# every case as well, so it takes the same fresh debug build as coverage and
# ownership -- the packaged binary may lag rust/ source.
node --test scripts/tsc-oracle.test.mjs
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/tsc-oracle-gate.mjs

sh -n scripts/*.sh
jq empty schema/*.json
jq empty fixtures/tsc-oracle/*.json
jq empty fixtures/upstream-parity/deviations.json
find pkg/contracts/bundled -type f -name '*.json' -exec jq empty {} +
node scripts/dialect-manifests.mjs validate
node scripts/check-bundled-contracts.mjs
node scripts/check-contract-pins.mjs
node scripts/generate-solid1-runtime-surface.mjs --check
node scripts/dialect-manifests.mjs check-composed-contracts
