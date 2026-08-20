RUST_TOOLCHAIN ?= 1.97
SOLID_CHECKER_BUILD_ID ?= dev
RUST_MANIFEST := rust/Cargo.toml

.PHONY: build build-typefacts build-rust build-checker-debug package test test-rust test-cli verify verify-performance corpus contract-corpus contract-conformance contracts contracts-check coverage coverage-update parity parity-update tsc-oracle tsc-oracle-provision tsc-ownership tsc-ownership-report ownership-gate clean

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

# A fresh source build for gates. Unlike build-rust this does not rebuild the
# pinned TypeFacts producer or overwrite the packaged/check-in binary under bin/.
build-checker-debug:
	cargo +$(RUST_TOOLCHAIN) build --manifest-path $(RUST_MANIFEST) \
	  -p solid-facts-backend --bin solid-checker-rust

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

# "Does TypeScript already report this?", as a checkable claim. Provisioning
# installs the audited Solid versions into rust/target/tsc-oracle and refuses
# to run on a version mismatch.
tsc-oracle-provision:
	node scripts/tsc-oracle.mjs provision --dialect all

# Needs the checker as well as the compiler: each case declares what TypeScript
# says *and* what this checker says about the same bytes.
tsc-oracle: tsc-oracle-provision build-checker-debug
	SOLID_CHECKER_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" node scripts/tsc-oracle-gate.mjs

# Compatibility target. Product ownership moved to ownership-gate after every
# retained upstream case was migrated into the product-owned manifest.
tsc-ownership: ownership-gate

# Which upstream cases are not valid TypeScript, and which findings look like
# duplicates. A discovery report, not a gate.
tsc-ownership-report: tsc-oracle-provision parity
	node scripts/parity-tsc-ownership.mjs --report

# Product-owned semantic cases. Unlike upstream parity, every expected finding
# carries its TypeScript-ownership disposition and exact source-relative span.
ownership-gate: tsc-oracle-provision build-checker-debug
	SOLID_CHECKER_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" node scripts/ownership-gate.mjs \
	  --require-retained --require-complete

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

contract-corpus: build-rust
	SOLID_CHECKER_NATIVE_BIN="$(CURDIR)/bin/solid-checker-rust" SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" node scripts/contract-corpus.mjs

contract-conformance:
	node scripts/check-bundled-contracts.mjs
	node scripts/check-contract-pins.mjs
	node scripts/generate-solid1-runtime-surface.mjs --check
	node scripts/dialect-manifests.mjs check-composed-contracts

# `contracts` and `contracts-check` read real installed packages: the exact
# versions the checked-in artifacts were generated against (solid-js 1.9.14
# aliased as solid-js-1x, solid-js and @solidjs/web 2.0.0-rc.0). The
# repository has no root package.json, so set each manifest-declared package
# path environment variable to node_modules holding those pins. CI's
# rust-engine job installs them into a scratch directory and runs
# `contracts-check` on every push and PR.
contracts:
	node scripts/dialect-manifests.mjs generate-contracts

contracts-check:
	node scripts/dialect-manifests.mjs check-contracts

clean:
	rm -rf bin dist rust/target .typefacts
