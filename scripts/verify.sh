#!/bin/sh
set -eu

rust_manifest=rust/Cargo.toml

cargo +1.97 fmt --manifest-path "$rust_manifest" --all -- --check
cargo +1.97 clippy --manifest-path "$rust_manifest" --workspace --all-targets

scripts/build-typefacts.sh
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  cargo +1.97 test --manifest-path "$rust_manifest" --workspace
cargo +1.97 build --release --manifest-path "$rust_manifest" \
  -p solid-facts-backend --bin solid-checker-session-bench
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  node benchmarks/verify-performance.mjs

npm ci --ignore-scripts --prefix packages/cli
npm test --prefix packages/cli

sh -n scripts/*.sh
jq empty schema/*.json pkg/contracts/bundled/*.json
node scripts/check-bundled-contracts.mjs
