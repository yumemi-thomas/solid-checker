RUST_TOOLCHAIN ?= 1.97
SOLID_CHECKER_BUILD_ID ?= dev
RUST_MANIFEST := rust/Cargo.toml
BUN ?= bun
# nextest is an optional local accelerator. Keep the built-in runner as the
# default because clean CI images do not necessarily have nextest installed;
# compare with `make CARGO_TEST_RUNNER='nextest run' test-rust` when available.
CARGO_TEST_RUNNER ?= test
TYPEFACTS_CERTIFICATION_ENV = \
	SOLID_TYPEFACTS_CERTIFICATION_SHA256="sha256:$$(shasum -a 256 bin/solid-typefacts | awk '{print $$1}')" \
	SOLID_TYPEFACTS_SOURCE_MANIFEST_SHA256="sha256:$$(node scripts/typefacts-source-identity.mjs --build-id "$(SOLID_CHECKER_BUILD_ID)" --digest)"

.PHONY: build build-typefacts build-rust build-checker-debug build-checker-release package test test-rust test-cli verify verify-delta verify-performance phase0-baseline phase16-report phase16-check phase18-audit phase19-audit phase20-ledger phase21-ledger compiler-facts-identity corpus contract-corpus contract-differential contract-conformance contracts contracts-check coverage coverage-update tsc-oracle tsc-oracle-provision tsc-ownership ownership-gate obligation-audit clean clean-verify

build: build-rust

# The local producer and Rust client share one source-manifest identity; the
# startup handshake rejects any build-id, protocol, or schema mismatch.
build-typefacts:
	TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" scripts/build-typefacts.sh

build-rust: build-typefacts
	mkdir -p bin
	$(TYPEFACTS_CERTIFICATION_ENV) SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --manifest-path $(RUST_MANIFEST) --workspace
	cp rust/target/debug/solid-checker-rust bin/solid-checker-rust

# A fresh source build for gates. Unlike build-rust this does not rebuild the
# pinned TypeFacts producer or overwrite the packaged/check-in binary under bin/.
build-checker-debug: build-typefacts
	$(TYPEFACTS_CERTIFICATION_ENV) cargo +$(RUST_TOOLCHAIN) build --manifest-path $(RUST_MANIFEST) \
	  -p solid-facts-backend --bin solid-checker-rust

# A fresh optimized checker for performance measurements. Like the debug gate
# build, this leaves the checked-in packaged binary under bin/ untouched.
build-checker-release: build-typefacts
	$(TYPEFACTS_CERTIFICATION_ENV) cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) \
	  -p solid-facts-backend --bin solid-checker-rust

package: build-typefacts
	$(TYPEFACTS_CERTIFICATION_ENV) SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) --workspace
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" $(BUN) scripts/package-rust.mjs --output dist/solid-checker

test: test-rust test-cli

test-rust: build-typefacts
	$(TYPEFACTS_CERTIFICATION_ENV) SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_TEST_BIN="$(CURDIR)/bin/solid-typefacts" SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" cargo +$(RUST_TOOLCHAIN) $(CARGO_TEST_RUNNER) --manifest-path $(RUST_MANIFEST) --workspace

test-cli:
	$(BUN) install --cwd packages/cli --ignore-scripts --no-progress --frozen-lockfile
	$(BUN) run --cwd packages/cli test

verify:
	scripts/verify.sh

phase0-baseline:
	$(BUN) scripts/package-contract-v2-phase0.mjs --check

phase16-report:
	cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) \
	  -p solid-facts-backend --bin solid-contract-phase16-bench
	$(BUN) scripts/package-contract-v2-phase16.mjs --write \
	  rust/target/release/solid-contract-phase16-bench

phase16-check:
	$(BUN) scripts/package-contract-v2-phase16.mjs --check

phase18-audit:
	$(BUN) scripts/package-contract-phase18.mjs

phase19-audit:
	$(BUN) scripts/package-contract-phase19.mjs

phase20-ledger:
	$(BUN) scripts/package-contract-v2-phase20-ledger.mjs --check

phase21-ledger:
	$(BUN) scripts/package-contract-v2-phase21-ledger.mjs --check

compiler-facts-identity:
	$(BUN) scripts/check-compiler-facts-identity.mjs

# AGENTS.md's "which check to run" table, mechanized: it maps every changed
# path to the checks that own it, prints each mapping decision, and appends the
# universal handoff set. A path the table does not claim escalates to the full
# `verify` above -- which remains the handoff authority regardless. Add
# `--dry-run` to see the plan without running it.
verify-delta:
	$(BUN) scripts/verify-delta.mjs

# "Does TypeScript already report this?", as a checkable claim. Provisioning
# installs the audited Solid versions into rust/target/tsc-oracle and refuses
# to run on a version mismatch.
tsc-oracle-provision:
	$(BUN) scripts/tsc-oracle.mjs provision --dialect all

# Needs the checker as well as the compiler: each case declares what TypeScript
# says *and* what this checker says about the same bytes.
tsc-oracle: tsc-oracle-provision build-checker-debug
	SOLID_CHECKER_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/tsc-oracle-gate.mjs

# The other half of the precision contract. The oracle holds a *reported*
# finding to being this checker's claim; this holds an *unreported* one to
# being a missing fact rather than an over-conservatism, by supplying the
# evidence and asking what changed.
obligation-audit: tsc-oracle-provision build-checker-debug
	SOLID_CHECKER_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/obligation-audit.mjs

# Compatibility target. Product ownership moved to ownership-gate after every
# retained upstream case was migrated into the product-owned manifest.
tsc-ownership: ownership-gate

# Product-owned semantic cases. Unlike upstream parity, every expected finding
# carries its TypeScript-ownership disposition and exact source-relative span.
ownership-gate: tsc-oracle-provision build-checker-debug
	SOLID_CHECKER_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/ownership-gate.mjs \
	  --require-retained --require-complete

# Fixture-findings snapshots: "no finding moved" as a checkable claim.
coverage: build-rust
	SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/coverage.mjs

coverage-update: build-rust
	SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/coverage.mjs --update

verify-performance: build-typefacts
	cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) -p solid-facts-backend --bin solid-checker-session-bench
	SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) benchmarks/verify-performance.mjs

corpus: build-rust
	scripts/run-solid-primitives-corpus.sh

contract-corpus: build-rust
	SOLID_CHECKER_NATIVE_BIN="$(CURDIR)/bin/solid-checker-rust" SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/contract-corpus.mjs

# Source-vs-contract parity requires the exact audited Solid typings used by
# the consumer side of the probe; provisioning is intentionally explicit.
contract-differential: build-checker-debug tsc-oracle-provision
	SOLID_CHECKER_NATIVE_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/contract-differential.mjs

contract-conformance:
	$(BUN) scripts/check-bundled-contracts.mjs
	$(BUN) scripts/check-contract-pins.mjs
	$(BUN) scripts/dialect-manifests.mjs check-composed-contracts

# Both targets replay the checked normalized authorities through the ordinary
# proof-and-receipt bundle issuer. Registry pin verification remains a separate
# live falsifier in `contract-conformance`.
contracts:
	$(BUN) scripts/dialect-manifests.mjs generate-contracts

contracts-check:
	$(BUN) scripts/dialect-manifests.mjs check-contracts

clean:
	rm -rf bin dist rust/target

# Reclaim the large handoff test/link artifacts without discarding release
# binaries, audited TypeScript installs, or content-addressed gate results.
clean-verify:
	cargo +$(RUST_TOOLCHAIN) clean --manifest-path $(RUST_MANIFEST) --profile verify

# Ecosystem benchmark: discovery, an offline pinned sentinel run, and the
# full-corpus run. See docs/ecosystem-benchmark.md for what these measure and
# why the runner is kept separate from contract-corpus.

# Enumerates every ecosystem family from the live npm registry and rewrites
# the manifest. Needs network access; review the printed diff before trusting
# a refreshed manifest, and never run this to silence a benchmark failure
# without reading what changed.
ecosystem-discover:
	$(BUN) scripts/ecosystem-benchmark/discover.mjs

ecosystem-benchmark-test:
	$(BUN) packages/cli/node_modules/vitest/vitest.mjs run \
	  --config packages/cli/vitest.config.mjs scripts/ecosystem-benchmark/*.test.mjs

# The pinned offline regression subset. Deliberately builds nothing: unlike
# tsc-oracle and ownership-gate below, this target trusts a debug binary
# already produced by `make build-checker-debug` (or CI's own build step)
# rather than rebuilding on every invocation. It points at the fresh
# rust/target/debug build rather than bin/solid-checker-rust, unlike
# contract-corpus and corpus above, because bin/solid-checker-rust is a
# checked-in artifact that can lag the source tree (AGENTS.md's "Stale
# binaries hide source changes"); a benchmark run must measure this commit's
# engine, not whatever was last packaged into bin/.
ecosystem-sentinel:
	SOLID_CHECKER_NATIVE_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" \
	  $(BUN) scripts/ecosystem-benchmark/run.mjs --sentinel

# The full discovered corpus is also the product-speed measurement, so it uses
# a fresh optimized binary. run.mjs keeps eight install/generation slots,
# uses the remaining host slots for certification, then expands the receipt
# drain to twenty after proposal work is claimed. Pass --concurrency or
# --certification-concurrency to compare another scheduling policy.
# Certification children share rust/target/registry-cache so the measured
# wall time is the checker's, not the registry's; --no-registry-cache
# restores fetch-everything-fresh acquisition.
ecosystem-benchmark: build-checker-release
	SOLID_CHECKER_NATIVE_BIN="$(CURDIR)/rust/target/release/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" \
	  $(BUN) scripts/ecosystem-benchmark/run.mjs --timeout 600 --attempt-certification \
	  --thresholds scripts/ecosystem-benchmark/phase16-thresholds.json

.PHONY: ecosystem-discover ecosystem-benchmark-test ecosystem-sentinel ecosystem-benchmark
