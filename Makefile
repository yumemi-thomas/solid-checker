RUST_TOOLCHAIN ?= 1.99.1
SOLID_CHECKER_BUILD_ID ?= dev
# This producer/certification label is not checker implementation identity.
# Daemon sockets and response handshakes bind the immutable loaded-image build
# identity (Mach-O UUID / Linux executing-inode digest), so same-path rebuilds
# with `dev` select a new actor without reading a replaceable executable path.
RUST_MANIFEST := rust/Cargo.toml
BUN ?= bun
# nextest is an optional local accelerator. Keep the built-in runner as the
# default because clean CI images do not necessarily have nextest installed;
# compare with `make CARGO_TEST_RUNNER='nextest run' test-rust` when available.
CARGO_TEST_RUNNER ?= test

# The Node executable the runtime resolver (ADR 0220) runs project config
# with, by real path.
PROBE_NODE ?= $(shell node -e 'process.stdout.write(require("fs").realpathSync(process.execPath))')
export SOLID_CHECKER_PROBE_NODE = $(PROBE_NODE)

.PHONY: build build-typefacts test-typefacts build-typefacts-transcript typefacts-differential typefacts-e2e build-rust build-checker-debug build-checker-release build-wasm-package build-wasm-parity-guests package test test-rust test-cli verify verify-delta verify-performance compiler-facts-identity coverage coverage-update repro-causes repro-budget repro-registry tsc-oracle tsc-oracle-provision runtime-oracle-install tsc-ownership ownership-gate obligation-audit wasm-parity wasm-parity-gate clean clean-verify ir-read-lint ir-edit-fuzz ir-edit-fuzz-corpus fuzz-focus fuzz-smoke

build: build-rust

# The Type Facts producer (apps/solid-typefacts, on tsrs; ADR 0365) and the Rust
# client share one source-manifest identity; the startup handshake rejects any
# build-id, protocol, or schema mismatch. The producer is its own Cargo
# workspace, lockfile and target directory (it needs tsrs optimised even in dev
# profile), built in release mode and copied to bin/solid-typefacts.
build-typefacts:
	TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" scripts/build-typefacts.sh

# Timing and packaging take the release producer, whatever TYPEFACTS_PROFILE
# the inner loop exported (the variable reaches their build-typefacts too).
build-checker-release package verify-performance: export TYPEFACTS_PROFILE := release

# fmt, clippy (native and the wasm32-wasip1 reactor) and the tests of the
# producer workspace. Its differential tests compare against a reference
# producer when one is available (SOLID_TYPEFACTS_REFERENCE_BIN, default the
# archived Go producer; docs/typefacts/differential.md) and skip otherwise.
test-typefacts: build-typefacts
	cargo +$(RUST_TOOLCHAIN) fmt --manifest-path apps/solid-typefacts/Cargo.toml -- --check
	TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) clippy --manifest-path apps/solid-typefacts/Cargo.toml --all-targets -- -D warnings
	TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) clippy --manifest-path apps/solid-typefacts/Cargo.toml --target wasm32-wasip1 --features reactor --lib -- -D warnings
	TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) test --manifest-path apps/solid-typefacts/Cargo.toml

# Differential harness for any two producer binaries (docs/typefacts/differential.md).
# The transcript tool decodes and replays recorded protocol streams with the
# Rust client's own decoder; it lives behind the typefacts crate's `transcript`
# feature, which the checker never enables. The REFERENCE records (default: the
# archived Go producer, or SOLID_TYPEFACTS_REFERENCE_BIN / TSGO_ARCHIVE); the
# CANDIDATE (default bin/solid-typefacts) replays.
#   make typefacts-differential                      record `coverage` through the reference, replay on the candidate, diff
#   make typefacts-differential GATE=ownership-gate  any of coverage, ownership-gate, tsc-oracle-gate,
#                                                    obligation-audit (corpus-sweep: record by hand)
#   make typefacts-differential DIFFERENTIAL_ARGS=--allow-unavailable
#   make typefacts-e2e                               `coverage` with the checker on the candidate, compare-only
GATE ?= coverage
TYPEFACTS_TRANSCRIPTS ?= $(CURDIR)/rust/target/differential/transcripts
DIFFERENTIAL_ARGS ?=
build-typefacts-transcript:
	TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --manifest-path $(RUST_MANIFEST) \
	  -p typefacts --features transcript --bin typefacts-transcript

typefacts-differential: build-typefacts build-typefacts-transcript build-checker-debug
	rm -rf "$(TYPEFACTS_TRANSCRIPTS)/$(GATE)"
	$(BUN) scripts/typefacts-differential.mjs record --gate $(GATE) --out "$(TYPEFACTS_TRANSCRIPTS)"
	$(BUN) scripts/typefacts-differential.mjs run "$(TYPEFACTS_TRANSCRIPTS)/$(GATE)" $(DIFFERENTIAL_ARGS)

typefacts-e2e: build-typefacts build-checker-debug
	$(BUN) scripts/typefacts-differential.mjs e2e

build-rust: build-typefacts
	mkdir -p bin
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --manifest-path $(RUST_MANIFEST) --workspace
	cp rust/target/debug/solid-checker-rust bin/solid-checker-rust

# A fresh source build for gates. Checks the producer stamp first and leaves
# the packaged checker under bin/ untouched.
build-checker-debug: build-typefacts
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --manifest-path $(RUST_MANIFEST) \
	  -p solid-facts-backend --bin solid-checker-rust

# A fresh optimized checker for performance measurements. Like the debug gate
# build, this leaves the checked-in packaged binary under bin/ untouched.
build-checker-release: build-typefacts
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) \
	  -p solid-facts-backend --bin solid-checker-rust --bin solid-checker-session-bench

package: build-typefacts
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) --workspace
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" $(BUN) scripts/package-rust.mjs --output dist/solid-checker

# The WASM package (packages/wasm): the Node-API module behind checkSync and
# planSync, then the owned check's two reactors (scripts/build-owned.mjs; the
# producer's is apps/solid-typefacts built as a wasm32-wasip1 cdylib). The
# Cargo builds run one after the other, with the native build's pins.
# Its own target directory, the one scripts/verify.sh builds the package in, so
# `make verify` and `make wasm-parity` share one build.
build-wasm-package: build-typefacts
	CARGO_TARGET_DIR="$(CURDIR)/rust/target/wasm-package" SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" $(BUN) run --cwd packages/wasm build

# The WASM parity gate's path-semantics control guests (S3a): a Rust program
# making raw WASI path calls and std calls. It is not part of the rust/
# workspace or the producer's source identity.
build-wasm-parity-guests:
	cargo +$(RUST_TOOLCHAIN) build --locked --manifest-path scripts/wasm-parity/guests/path-control/Cargo.toml \
	  --target wasm32-wasip1 --release --target-dir rust/target/wasm-parity-guests

# The owned check, packed and installed in isolation, against the native
# ordinary check (scripts/wasm-parity.mjs). The
# gate builds nothing: the package, the native checker and the bridge test's
# recorded update frames are made here first, and the gate fails on any it
# cannot find. Uncached, by design. `wasm-parity-gate` is the gate over a
# package already built (CI's wasm-package job builds it first).
wasm-parity: build-wasm-package wasm-parity-gate

wasm-parity-gate: build-checker-debug tsc-oracle-provision build-wasm-parity-guests
	scratch=$$(mktemp -d "$${TMPDIR:-/tmp}/sc-wasm-parity.XXXXXX") && mkdir -p "$$scratch/n2-tmp" && \
	  TMPDIR="$$scratch/n2-tmp" TYPEFACTS_BRIDGE_FRAMES="$$scratch/update-frames" \
	  SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" \
	  TYPEFACTS_TEST_BIN="$(CURDIR)/bin/solid-typefacts" cargo +$(RUST_TOOLCHAIN) test --manifest-path $(RUST_MANIFEST) \
	  -p typefacts --test bridge_process -- --exact bridged_answers_equal_process_answers && \
	  SOLID_CHECKER_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" \
	  $(BUN) scripts/wasm-parity.mjs --scratch "$$scratch" --update-frames "$$scratch/update-frames"

test: test-rust test-cli

test-rust: build-typefacts
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_TEST_BIN="$(CURDIR)/bin/solid-typefacts" SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" cargo +$(RUST_TOOLCHAIN) $(CARGO_TEST_RUNNER) --manifest-path $(RUST_MANIFEST) --workspace

test-cli:
	$(BUN) install --cwd packages/cli --ignore-scripts --no-progress --frozen-lockfile
	$(BUN) run --cwd packages/cli test

# Exact/filter-based library tests, with the same certification inputs as the
# full suite. The driver rejects missing or empty selections before testing.
# Enables the repository's tracked git hooks. `core.hooksPath` is local
# config, so a repository cannot turn its own hooks on — every clone runs this
# once. The only hook today refuses a commit staging a file over 5 MB.
hooks:
	git config core.hooksPath .githooks
	@echo "git hooks enabled from .githooks"

.PHONY: hooks

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
	SOLID_CHECKER_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" TYPEFACTS_BUILD_ID="$(SOLID_CHECKER_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) clippy --manifest-path $(RUST_MANIFEST) --workspace --all-targets -- -D warnings

verify:
	scripts/verify.sh

compiler-facts-identity:
	$(BUN) scripts/check-compiler-facts-identity.mjs

# Maps every changed path to the checks that own it, prints each mapping
# decision, and appends the universal handoff set. A path no row claims
# escalates to the full `verify` above -- which remains the handoff authority
# regardless. `BASE=<rev>` diffs against the commit the slice started from
# instead of origin/main; `bun scripts/verify-delta.mjs --dry-run` shows the
# plan without running it.
verify-delta:
	$(BUN) scripts/verify-delta.mjs $(if $(BASE),--base $(BASE))

# The runtime oracle's pinned Solid 2.0.0-rc.17 install (ADR 0453): the committed
# manifest and lockfile in scripts/runtime-oracle-install, reproduced by `npm ci`.
# scripts/runtime-oracle.mjs defaults --install to it.
runtime-oracle-install:
	cd scripts/runtime-oracle-install && npm ci

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

# The differential fuzzer's short loop (scripts/fuzz/focus.mjs): the six grids restricted to CODES, on the
# Rust executor and the Chrome host, each with its oracle sample; exits 1 when anything needs a look.
#   make fuzz-focus CODES=STRICT_READ_UNTRACKED[,...] [FUZZ_GRIDS=mx-base,lg-base] [FUZZ_FOCUS_ARGS="-- --per-cell 8"]
fuzz-focus: build-checker-release
	@test -n "$(CODES)" || { echo "make fuzz-focus CODES=A[,B]: the codes the change touches"; exit 2; }
	node scripts/fuzz/focus.mjs --codes "$(CODES)" $(if $(FUZZ_GRIDS),--grids "$(FUZZ_GRIDS)") \
	  --checker-bin "$(CURDIR)/rust/target/release/solid-checker-rust" --typefacts "$(CURDIR)/bin/solid-typefacts" $(FUZZ_FOCUS_ARGS)

# The fuzzer's smoke set (scripts/fuzz/smoke.mjs): every grid restricted to its manifest (scripts/fuzz/smoke/), a few
# programs covering every behaviour and axis pair of the full grids; after any checker change. Exits 1 on anything to look at.
fuzz-smoke: build-checker-release
	node scripts/fuzz/focus.mjs --smoke \
	  --checker-bin "$(CURDIR)/rust/target/release/solid-checker-rust" --typefacts "$(CURDIR)/bin/solid-typefacts" $(FUZZ_FOCUS_ARGS)

# The retained IR's safety net (scripts/ir-read-lint.mjs, scripts/ir-edit-fuzz.mjs).
# The lint ratchets raw cross-file reads against scripts/ir-read-lint.allowlist.json.
# The fixture budget edits copies of fixture projects and compares the daemon's
# answer with a fresh one after every edit (seconds; `make verify` runs it).
# The corpus budget is the long manual one, on a project edited in place and
# restored byte-exact, against the release checker (the debug build inflates
# certification about 19x):
#   make ir-edit-fuzz-corpus IR_FUZZ_PROJECT=<corpus>/apps/solid-groove/tsconfig.json IR_FUZZ_STEPS=30
IR_FUZZ_STEPS ?= 6
IR_FUZZ_SEED ?= 1
IR_FUZZ_PROJECT ?=
ir-read-lint:
	$(BUN) scripts/ir-read-lint.mjs

ir-edit-fuzz: build-checker-debug
	SOLID_CHECKER_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/ir-edit-fuzz.mjs \
	  --fixtures --steps $(IR_FUZZ_STEPS) --seed $(IR_FUZZ_SEED)
	SOLID_CHECKER_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/ir-edit-fuzz.mjs \
	  --fixtures --family retype --steps 4 --seed $(IR_FUZZ_SEED)

ir-edit-fuzz-corpus: build-checker-release
	test -n "$(IR_FUZZ_PROJECT)" || { echo "set IR_FUZZ_PROJECT=<tsconfig.json>"; exit 2; }
	SOLID_CHECKER_BIN="$(CURDIR)/rust/target/release/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/ir-edit-fuzz.mjs \
	  --project "$(IR_FUZZ_PROJECT)" --steps $(IR_FUZZ_STEPS) --seed $(IR_FUZZ_SEED)

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
# Coverage runs the fresh debug checker, like the ownership gate: it builds one
# binary rather than the workspace and leaves the packaged bin/ checker alone.
coverage: build-checker-debug
	SOLID_CHECKER_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/coverage.mjs

coverage-update: build-checker-debug
	SOLID_CHECKER_BIN="$(CURDIR)/rust/target/debug/solid-checker-rust" \
	  SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) scripts/coverage.mjs --update

# The uncertifiable reproduction set (ADR 0446): every case still produces its
# expected results with the cause the classifier reads, and the set stays in
# its budget. The corpus side (scripts/uncertifiable-causes.mjs --report) needs
# a sweep and runs by hand; CONTRIBUTING.md, "Adding a repo to the reproduction set".
repro-causes: build-checker-debug
	$(BUN) scripts/uncertifiable-causes.mjs --repro fixtures/uncertifiable-repro \
	  --cases fixtures/uncertifiable-repro/cases.json \
	  --checker "$(CURDIR)/rust/target/debug/solid-checker-rust" --typefacts "$(CURDIR)/bin/solid-typefacts"

repro-budget:
	$(BUN) scripts/repro-budget.mjs

# Opt-in, needs network: not a dependency of verify.
repro-registry:
	$(BUN) scripts/repro-registry-check.mjs

verify-performance: build-typefacts
	cargo +$(RUST_TOOLCHAIN) build --release --manifest-path $(RUST_MANIFEST) -p solid-facts-backend --bin solid-checker-session-bench
	SOLID_TYPEFACTS_BIN="$(CURDIR)/bin/solid-typefacts" $(BUN) benchmarks/verify-performance.mjs

clean:
	rm -rf bin dist rust/target

# Reclaim the target directories only `make verify` and `make verify-delta`
# build in (check-only lint, the WASM package, and the retired `verify` profile)
# without discarding the edit loop's build, release binaries, audited
# TypeScript installs, or content-addressed gate results.
clean-verify:
	rm -rf rust/target/lint rust/target/wasm-package rust/target/verify
