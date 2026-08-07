RUST_TOOLCHAIN ?= 1.97
SOLID_CHECKER_BUILD_ID ?= dev
RUST_MANIFEST := rust/Cargo.toml
SOLID_1_PACKAGE ?= node_modules/solid-js-1x
SOLID_2_PACKAGE ?= node_modules/solid-js
SOLIDJS_WEB_PACKAGE ?= node_modules/@solidjs/web

.PHONY: build build-typefacts build-rust package test test-rust test-cli verify verify-performance corpus contract-conformance contracts contracts-check coverage coverage-update parity parity-update clean

build: build-rust

# The producer lives in yumemi-thomas/solid-ts-facts and is built from the
# revision `rust/Cargo.toml` pins the client to; the startup handshake rejects
# any other pairing.
build-typefacts:
	TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" scripts/build-typefacts.sh

build-rust: build-typefacts
	mkdir -p bin
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --manifest-path $(RUST_MANIFEST) --workspace
	cp rust/target/debug/solid-checker-rust bin/solid-checker-rust

package: build-typefacts
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) --workspace
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" node scripts/package-rust.mjs --output dist/solid-checker

test: test-rust test-cli

test-rust: build-typefacts
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" cargo +$(RUST_TOOLCHAIN) test --manifest-path $(RUST_MANIFEST) --workspace

test-cli:
	npm ci --ignore-scripts --prefix packages/cli
	npm test --prefix packages/cli

verify:
	scripts/verify.sh

# Fixture-findings snapshots: "no finding moved" as a checkable claim.
coverage: build-rust
	SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" node scripts/coverage.mjs

coverage-update: build-rust
	SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" node scripts/coverage.mjs --update

# eslint-plugin-solid's own 465 test cases, with every deviation declared.
parity: build-rust
	SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" node scripts/parity.mjs

parity-update: build-rust
	SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" node scripts/parity.mjs --update

verify-performance: build-typefacts
	cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) -p solid-facts-backend --bin solid-checker-session-bench
	SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" node benchmarks/verify-performance.mjs

corpus: build-rust
	scripts/run-solid-primitives-corpus.sh

contract-conformance:
	node scripts/check-bundled-contracts.mjs
	node scripts/generate-bundled-solid1-contract.mjs --check

contracts:
	cargo +$(RUST_TOOLCHAIN) run --manifest-path $(RUST_MANIFEST) -p solid-facts-backend --bin solid-contract-gen -- --package $(SOLID_1_PACKAGE) --dialect solid-js-1x --out rust/crates/solid-dialect/contracts/solid-js-1x.json --index-out rust/crates/solid-dialect/src/exports/solid_js_1x.rs
	cargo +$(RUST_TOOLCHAIN) run --manifest-path $(RUST_MANIFEST) -p solid-facts-backend --bin solid-contract-gen -- --package $(SOLID_2_PACKAGE) --dialect solid-js --out rust/crates/solid-dialect/contracts/solid-js.json --index-out rust/crates/solid-dialect/src/exports/solid_js_2.rs
	cargo +$(RUST_TOOLCHAIN) run --manifest-path $(RUST_MANIFEST) -p solid-facts-backend --bin solid-contract-gen -- --package $(SOLIDJS_WEB_PACKAGE) --dialect solidjs-web --out rust/crates/solid-dialect/contracts/solidjs-web.json --index-out rust/crates/solid-dialect/src/exports/solidjs_web.rs

contracts-check:
	cargo +$(RUST_TOOLCHAIN) run --manifest-path $(RUST_MANIFEST) -p solid-facts-backend --bin solid-contract-gen -- --package $(SOLID_1_PACKAGE) --dialect solid-js-1x --out rust/crates/solid-dialect/contracts/solid-js-1x.json --index-out rust/crates/solid-dialect/src/exports/solid_js_1x.rs --check
	cargo +$(RUST_TOOLCHAIN) run --manifest-path $(RUST_MANIFEST) -p solid-facts-backend --bin solid-contract-gen -- --package $(SOLID_2_PACKAGE) --dialect solid-js --out rust/crates/solid-dialect/contracts/solid-js.json --index-out rust/crates/solid-dialect/src/exports/solid_js_2.rs --check
	cargo +$(RUST_TOOLCHAIN) run --manifest-path $(RUST_MANIFEST) -p solid-facts-backend --bin solid-contract-gen -- --package $(SOLIDJS_WEB_PACKAGE) --dialect solidjs-web --out rust/crates/solid-dialect/contracts/solidjs-web.json --index-out rust/crates/solid-dialect/src/exports/solidjs_web.rs --check

clean:
	rm -rf bin dist rust/target .typefacts
