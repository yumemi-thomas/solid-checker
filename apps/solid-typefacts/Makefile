RUST_TOOLCHAIN ?= 1.99.1
TYPEFACTS_BUILD_ID ?= dev
REPOSITORY_ROOT := $(abspath ../..)
TYPEFACTS_BIN ?= $(REPOSITORY_ROOT)/bin/solid-typefacts
SOLID_CHECKER_REPO ?= $(REPOSITORY_ROOT)
MEMORY_PROJECT ?= /tmp/bench-corpus-5k/tsconfig.json
MEMORY_EDIT ?= /tmp/bench-corpus-5k/mod4383.tsx
MEMORY_MAX_PHYSICAL_MIB ?= 1200
SOLID_CHECKER_BIN ?= $(SOLID_CHECKER_REPO)/rust/target/release/solid-checker-rust
SOLID_CHECKER_SESSION_BENCH ?= $(SOLID_CHECKER_REPO)/rust/target/release/solid-checker-session-bench

.PHONY: build test fmt clippy benchmark-memory-5k

build:
	TYPEFACTS_BUILD_ID="$(TYPEFACTS_BUILD_ID)" "$(REPOSITORY_ROOT)/scripts/build-typefacts.sh" "$(TYPEFACTS_BIN)"

fmt:
	cargo +$(RUST_TOOLCHAIN) fmt -- --check

clippy:
	TYPEFACTS_BUILD_ID="$(TYPEFACTS_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) clippy --all-targets -- -D warnings
	TYPEFACTS_BUILD_ID="$(TYPEFACTS_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) clippy --target wasm32-wasip1 --features reactor --lib -- -D warnings

test: fmt clippy build
	TYPEFACTS_TEST_BIN="$(TYPEFACTS_BIN)" TYPEFACTS_BUILD_ID="$(TYPEFACTS_BUILD_ID)" cargo +$(RUST_TOOLCHAIN) test

# Exact Solid Checker corpus gate. Override the paths when the checker checkout
# or generated corpus lives elsewhere.
benchmark-memory-5k: build
	test -x "$(SOLID_CHECKER_BIN)"
	test -x "$(SOLID_CHECKER_SESSION_BENCH)"
	test -f "$(MEMORY_PROJECT)"
	SOLID_CHECKER_BIN="$(SOLID_CHECKER_BIN)" \
		SOLID_TYPEFACTS_BIN="$(TYPEFACTS_BIN)" \
		SOLID_CHECKER_MEMORY_PROJECT="$(MEMORY_PROJECT)" \
		node "$(SOLID_CHECKER_REPO)/benchmarks/measure-retained-memory.mjs" \
			--vmmap-all --idle-secs=30 --max-physical-mib="$(MEMORY_MAX_PHYSICAL_MIB)"
	"$(SOLID_CHECKER_SESSION_BENCH)" --project "$(MEMORY_PROJECT)" \
		--typefacts "$(TYPEFACTS_BIN)" --iterations 30 --warmups 3
	"$(SOLID_CHECKER_SESSION_BENCH)" --project "$(MEMORY_PROJECT)" \
		--typefacts "$(TYPEFACTS_BIN)" --iterations 30 --warmups 3 \
		--edit "$(MEMORY_EDIT)" --edit-mode same-span-body
