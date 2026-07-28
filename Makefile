RUST_TOOLCHAIN ?= 1.97
TYPEFACTS_BUILD_ID ?= dev
TYPEFACTS_BIN := $(CURDIR)/bin/solid-typefacts
SOLID_CHECKER_REPO ?= ../Github/solid-checker
MEMORY_PROJECT ?= /tmp/bench-corpus-5k/tsconfig.json
MEMORY_EDIT ?= /tmp/bench-corpus-5k/mod4383.tsx
MEMORY_MAX_PHYSICAL_MIB ?= 1200
MEMORY_GOCACHE ?= /tmp/solid-ts-facts-memory-go-cache
SOLID_CHECKER_BIN ?= $(SOLID_CHECKER_REPO)/rust/target/release/solid-checker-rust
SOLID_CHECKER_SESSION_BENCH ?= $(SOLID_CHECKER_REPO)/rust/target/release/solid-checker-session-bench

.PHONY: build test test-go test-rust fmt vet benchmark-memory benchmark-memory-5k

build:
	mkdir -p bin
	go build -ldflags "-X main.buildID=$(TYPEFACTS_BUILD_ID)" -o "$(TYPEFACTS_BIN)" ./cmd/solid-typefacts

test: fmt vet test-go test-rust

fmt:
	test -z "$$(gofmt -l cmd internal shims)"
	cargo +$(RUST_TOOLCHAIN) fmt --all -- --check

vet:
	go vet ./...
	cargo +$(RUST_TOOLCHAIN) clippy --workspace --all-targets

test-go:
	go test -race ./...

test-rust: build
	TYPEFACTS_TEST_BIN="$(TYPEFACTS_BIN)" TYPEFACTS_BUILD_ID="$(TYPEFACTS_BUILD_ID)" \
		cargo +$(RUST_TOOLCHAIN) test --workspace

# Fast, repository-owned structural and latency gates. The symbol-set test
# catches the exact-growth regression that accounted for most cold allocation.
benchmark-memory:
	GOCACHE="$(MEMORY_GOCACHE)" go test ./internal/typefacts -run 'Test(SymbolHandleSetGrowsGeometricallyDuringColdInterning|RetainedContributionSharesCanonicalEntityBacking|SemanticTableRetainsOnlySourceDigests|RetainedDemandStoreOwnsCompactRunsWithoutExpandedRows|SemanticMaterializationReleasesExpandedCompactDemands|CompactDemandsRoundTrip)'
	GOCACHE="$(MEMORY_GOCACHE)" go test ./internal/typefacts -run '^$$' \
		-bench 'Benchmark(ColdSymbolHandleSetMemory|FullTableAnalyzeAtScale|AnalyzeAfterLeafEditAtScale)$$' \
		-benchmem -count=5

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
