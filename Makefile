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

# The runtime-probe harness image and the Node runtime it is launched with.
# Both are compiled into the verifier; a build without them refuses probe
# authority, so a nonempty probe-gate schedule cannot certify at all rather
# than certifying an unvetoed closure.
#
# PROBE_NODE must name the *real path* of the Node executable, because the
# adapter refuses a symlink: a symlink is a name that can be repointed at other
# bytes after the pin was taken. Override it to pin a different interpreter
# (`make PROBE_NODE=/usr/local/bin/node …`); CI pins whichever `node` its
# toolchain step installed, which is what the default expression resolves.
PROBE_NODE ?= $(shell node -e 'process.stdout.write(require("fs").realpathSync(process.execPath))')
#
# `PROBE_NODE` is exported alongside the digests, not only consumed here: the
# probe-gate tracers and `scripts/check-bundled-contracts.mjs` would otherwise
# re-resolve `node` from `PATH` and could pin — or skip on — a different
# executable than the one this build hashed.
PROBE_HARNESS_ENV = \
	PROBE_NODE="$(PROBE_NODE)" \
	SOLID_CHECKER_PROBE_HARNESS_SHA256="sha256:$$(node scripts/probe-harness-source-identity.mjs --build-id "$(SOLID_CHECKER_BUILD_ID)" --write-stamp --digest)" \
	SOLID_CHECKER_PROBE_NODE_SHA256="sha256:$$(shasum -a 256 "$(PROBE_NODE)" | awk '{print $$1}')" \
	$(PROBE_BROWSER_ENV)

# ADR 0033: the optional browser pin for the controlled browser execution
# profile. `PROBE_BROWSER` names the *real path* of a headless-shell executable
# (`make PROBE_BROWSER=/path/to/chrome-headless-shell …`); the pin compiled into
# the verifier is the tree digest of its directory, in `hash_tree`'s framing
# (scripts/probe-browser-identity.mjs). Empty by default: a build without it
# refuses the browser profile only, and every Node profile is unaffected. The
# browser tracers skip when it is unset and fail loudly under
# `SOLID_CHECKER_EXPECT_BROWSER_PIN=1`, which is set only when it is.
PROBE_BROWSER ?=
PROBE_BROWSER_ENV = $(if $(PROBE_BROWSER),PROBE_BROWSER="$(PROBE_BROWSER)" SOLID_CHECKER_PROBE_BROWSER_SHA256="sha256:$$(node scripts/probe-browser-identity.mjs "$(PROBE_BROWSER)")" SOLID_CHECKER_EXPECT_BROWSER_PIN=1,)

CERTIFICATION_ENV = $(TYPEFACTS_CERTIFICATION_ENV) $(PROBE_HARNESS_ENV)

# Turns a silently skipped probe assertion into a loud failure, exactly as
# `scripts/verify.sh` does. Every probe-gate tracer returns early when the
# binary was compiled without the pins or when no Node executable can be
# resolved, so the fast loop would otherwise report a green run that asserted
# nothing about the binding.
#
# Conditional on `PROBE_NODE`, which is empty when `node` is not installed: on
# such a machine the pins cannot be computed at all, so demanding them would
# fail the build rather than the assertion. **That is the stated limit** — a
# `make test-rust` without Node still skips every tracer, and only
# `scripts/verify.sh` (which exits 127 without Node) closes it.
PROBE_EXPECT_PINS = $(if $(PROBE_NODE),SOLID_CHECKER_EXPECT_PROBE_PINS=1,)

# The rc.3 archives the Solid 2.0 negative table's implementation-audited rows
# quote by byte range. The cited ranges are checked into
# `rust/crates/solid-dialect/audited-slices/` and their digests are verified
# with no install; this arms the stronger arm, which re-reads the real archive
# and asserts the checked-in slice is still exactly those bytes of the pinned
# file. `PROBE_EXPECT_PINS` above makes its absence a loud failure, so any
# target that sets that must set this too — which is why `test-rust` now
# depends on `tsc-oracle-provision` (idempotent: it short-circuits on a tree
# that already passes the version check).
RC3_ARCHIVE_ENV = SOLID_CHECKER_RC3_ARCHIVE_ROOT="$(CURDIR)/rust/target/tsc-oracle/v2/node_modules"

.PHONY: build build-typefacts build-rust build-checker-debug build-checker-release package test test-rust test-probe-harness test-cli verify verify-delta verify-performance phase0-baseline phase16-report phase16-check phase18-audit phase19-audit phase20-ledger phase21-ledger compiler-facts-identity corpus contract-corpus contract-differential contract-conformance contracts contracts-check coverage coverage-update tsc-oracle tsc-oracle-provision tsc-ownership ownership-gate obligation-audit clean clean-verify

build: build-rust

# The local producer and Rust client share one source-manifest identity; the
# startup handshake rejects any build-id, protocol, or schema mismatch.
build-typefacts:
	TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" scripts/build-typefacts.sh

build-rust: build-typefacts
	mkdir -p bin
	$(CERTIFICATION_ENV) SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --manifest-path $(RUST_MANIFEST) --workspace
	cp rust/target/debug/solid-checker-rust bin/solid-checker-rust

# A fresh source build for gates. Checks the producer stamp first and leaves
# the packaged checker under bin/ untouched.
build-checker-debug: build-typefacts
	$(CERTIFICATION_ENV) SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --manifest-path $(RUST_MANIFEST) \
	  -p solid-facts-backend --bin solid-checker-rust

# A fresh optimized checker for performance measurements. Like the debug gate
# build, this leaves the checked-in packaged binary under bin/ untouched.
build-checker-release: build-typefacts
	$(CERTIFICATION_ENV) SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) \
	  -p solid-facts-backend --bin solid-checker-rust

package: build-typefacts
	$(CERTIFICATION_ENV) SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) --workspace
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" $(BUN) scripts/package-rust.mjs --output dist/solid-checker

test: test-rust test-cli

test-rust: build-typefacts tsc-oracle-provision
	$(CERTIFICATION_ENV) $(PROBE_EXPECT_PINS) $(RC3_ARCHIVE_ENV) SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_TEST_BIN="$(CURDIR)/bin/solid-typefacts" SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" cargo +$(RUST_TOOLCHAIN) $(CARGO_TEST_RUNNER) --manifest-path $(RUST_MANIFEST) --workspace

# The probe-harness binding on its own, for the fast loop and for
# `verify-delta`'s harness-script row.
#
# It must go through this Makefile rather than a bare `cargo test`: the two
# harness digests are read with `option_env!`, so a bare invocation compiles a
# binary that refuses probe authority and turns every assertion below into an
# early return. The `probe` filter selects `probe_harness::tests::*`, the
# `runtime_probes` evaluator tests, and `contract_certification::tests::the_probe_gate_tracer_*`.
test-probe-harness: build-typefacts
	$(CERTIFICATION_ENV) $(PROBE_EXPECT_PINS) SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_TEST_BIN="$(CURDIR)/bin/solid-typefacts" SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" cargo +$(RUST_TOOLCHAIN) $(CARGO_TEST_RUNNER) --manifest-path $(RUST_MANIFEST) -p solid-facts-backend --lib probe

test-cli:
	$(BUN) install --cwd packages/cli --ignore-scripts --no-progress --frozen-lockfile
	$(BUN) run --cwd packages/cli test

# Exact/filter-based library tests, with the same certification inputs as the
# full suite. The driver rejects missing or empty selections before testing.
.PHONY: test-focused
test-focused: export TEST := $(TEST)
test-focused: export TEST_EXACT := $(if $(TEST_EXACT),$(TEST_EXACT),0)
test-focused: export TEST_PACKAGE := $(if $(TEST_PACKAGE),$(TEST_PACKAGE),solid-facts-backend)
test-focused: export SOLID_CHECKER_BUILD_ID := $(SOLID_CHECKER_BUILD_ID)
test-focused:
	$(BUN) scripts/test-focused.mjs

.PHONY: verify-fast
verify-fast: build-typefacts
	cargo +$(RUST_TOOLCHAIN) fmt --manifest-path $(RUST_MANIFEST) --all -- --check
	$(CERTIFICATION_ENV) SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) clippy --manifest-path $(RUST_MANIFEST) --workspace --all-targets -- -D warnings

.PHONY: ecosystem-package
ecosystem-package: export PACKAGE := $(PACKAGE)
ecosystem-package: export ECOSYSTEM_PROFILE := $(if $(ECOSYSTEM_PROFILE),$(ECOSYSTEM_PROFILE),release)
ecosystem-package: export SOLID_CHECKER_BUILD_ID := $(SOLID_CHECKER_BUILD_ID)
ecosystem-package:
	$(BUN) scripts/ecosystem-benchmark/package.mjs

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
	PROBE_NODE="$(PROBE_NODE)" $(BUN) scripts/check-bundled-contracts.mjs
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
# Both certifying targets hand the runner the checked-in recipe corpus
# (scripts/ecosystem-benchmark/probe-recipes): a `creates` candidate whose claim
# a recipe names is vetoed and closed, one no recipe names is served by a
# synthesized veto or withheld by name (ADR 0036). Without it every candidate in
# the corpus was withheld as `noRecipe` (1664 on 2026-09-05) and the pinned
# report described no closure at all.
ECOSYSTEM_PROBE_RECIPES := scripts/ecosystem-benchmark/probe-recipes

ecosystem-benchmark: build-checker-release
	SOLID_CHECKER_NATIVE_BIN="$(CURDIR)/rust/target/release/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" \
	  $(BUN) scripts/ecosystem-benchmark/run.mjs --timeout 600 --attempt-certification \
	  --probe-recipe-corpus "$(ECOSYSTEM_PROBE_RECIPES)" \
	  --thresholds scripts/ecosystem-benchmark/phase16-thresholds.json

# The certification regression gate: the same full run as ecosystem-benchmark,
# compared against the pinned benchmarks/ecosystem/report.json and failing on
# any row the pin certified that this commit does not. It writes its reports
# under rust/target so the pin is never moved by a gate run; repinning stays a
# deliberate `make ecosystem-benchmark`. This is what a change to the certifier,
# the producer, the generator, or the runner runs before it lands: `make verify`
# does not include it (it needs the registry and several minutes of compute),
# and the three-row corpus cannot see a receipt lost elsewhere in the corpus --
# twelve were lost between the 2026-09-04 pin and 01b84ada with every gate green.
ecosystem-regression: build-checker-release
	mkdir -p "$(CURDIR)/rust/target/ecosystem-regression"
	SOLID_CHECKER_NATIVE_BIN="$(CURDIR)/rust/target/release/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" \
	  $(BUN) scripts/ecosystem-benchmark/run.mjs --timeout 600 --attempt-certification \
	  --probe-recipe-corpus "$(ECOSYSTEM_PROBE_RECIPES)" \
	  --baseline benchmarks/ecosystem/report.json \
	  --thresholds scripts/ecosystem-benchmark/certification-regression-thresholds.json \
	  --json "$(CURDIR)/rust/target/ecosystem-regression/report.json" \
	  --markdown "$(CURDIR)/rust/target/ecosystem-regression/report.md"

.PHONY: ecosystem-discover ecosystem-benchmark-test ecosystem-sentinel ecosystem-benchmark ecosystem-regression
