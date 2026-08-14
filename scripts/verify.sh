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
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/parity.mjs

cargo +1.97 build --release --manifest-path "$rust_manifest" \
  -p solid-facts-backend --bin solid-checker-session-bench
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  node benchmarks/verify-performance.mjs

npm ci --ignore-scripts --prefix packages/cli
npm test --prefix packages/cli

sh -n scripts/*.sh
jq empty schema/*.json
find pkg/contracts/bundled -type f -name '*.json' -exec jq empty {} +
node scripts/dialect-manifests.mjs validate
node scripts/check-bundled-contracts.mjs
node scripts/dialect-manifests.mjs check-composed-contracts
