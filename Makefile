RUST_TOOLCHAIN ?= 1.97
TYPEFACTS_BUILD_ID ?= dev
TYPEFACTS_BIN := $(CURDIR)/bin/solid-typefacts

.PHONY: build test test-go test-rust fmt vet

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
