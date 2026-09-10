#!/bin/sh
set -eu

rust_manifest=rust/Cargo.toml
cargo_profile=verify
checker_bin="$PWD/rust/target/$cargo_profile/solid-checker-rust"
rust_test_runner="${SOLID_CHECKER_RUST_TEST_RUNNER:-test}"
case "$rust_test_runner" in
  auto)
    if command -v cargo-nextest >/dev/null 2>&1; then
      rust_test_runner=nextest
    else
      rust_test_runner=test
    fi
    ;;
  nextest|test) ;;
  *)
    echo "make verify: SOLID_CHECKER_RUST_TEST_RUNNER must be auto, nextest, or test" >&2
    exit 2
    ;;
esac

# `bun` is now required before the first cargo step, not only by the Bun gates
# further down: the timing clock below is a `bun -e`, and under `set -e` a
# failed command substitution in an assignment aborts the script with a bare
# "bun: command not found" that says nothing about what wanted it. Fail fast
# with a sentence instead.
if ! command -v bun >/dev/null 2>&1; then
  echo "make verify: bun is required (the per-step clock, the coverage/oracle/contract gates," >&2
  echo "  and the Bun steps all run under it). Install Bun and re-run." >&2
  exit 127
fi

# ---------------------------------------------------------------------------
# Per-step wall time.
#
# `make verify` is one long sequence of unequal steps, and without a breakdown
# every discussion about its cost is a guess. Each `step <name>` closes the
# previous step (printing its wall time) and opens the next. The combined test
# stage waits for both suites and reports each status; `set -eu` prevents later
# gates from running if either suite fails.
#
# The clock is a single `bun -e` per boundary (bun is already required by
# steps below), so one read serves as the previous step's end and the next
# step's start. The run anchor and final summary each take one additional read.
# `date` is not used because POSIX `date` has no sub-second field and several
# steps finish in tens of milliseconds.
#
# `SOLID_CHECKER_GATE_CACHE=0` in the environment forces the content-addressed
# gate caches (coverage, contract corpus, registry pins) to recompute
# everything; `SOLID_CHECKER_GATE_CONCURRENCY=<N>` overrides the gates' default
# fan-out of min(cores, 8).
# ---------------------------------------------------------------------------

epoch_ms() { bun -e 'process.stdout.write(String(Date.now()))'; }

timings=""
step_name=""
run_start=$(epoch_ms)
step_start=$run_start

step() {
  if [ -n "$step_name" ]; then
    step_now=$(epoch_ms)
    step_ms=$((step_now - step_start))
    timings="$timings$step_name $step_ms
"
    printf '=== step %-22s %d.%03ds\n' "$step_name" $((step_ms / 1000)) $((step_ms % 1000))
    step_start=$step_now
  fi
  step_name="${1-}"
}

# A failure exits before its step is closed, so the step name is the one thing
# the timing machinery still owes the reader.
on_exit() {
  status=$?
  if [ "$status" -ne 0 ] && [ -n "$step_name" ]; then
    printf '\n=== FAILED during step %s (exit %d)\n' "$step_name" "$status" >&2
  fi
}
trap on_exit EXIT

summarize() {
  total_now=$(epoch_ms)
  total_ms=$((total_now - run_start))
  printf '\n=== make verify: per-step wall time ===\n'
  printf '%s' "$timings" | awk -v total="$total_ms" '
    { name[NR] = $1; ms[NR] = $2; sum += $2 }
    END {
      printf "  %-22s %9s %8s\n", "step", "seconds", "% total"
      for (i = 1; i <= NR; i++) printf "  %-22s %9.2f %7.1f%%\n", name[i], ms[i] / 1000, 100 * ms[i] / total
      printf "  %-22s %9.2f %7.1f%%\n", "(sum of steps)", sum / 1000, 100 * sum / total
      printf "  %-22s %9.2f %7.1f%%\n", "TOTAL", total / 1000, 100
    }'
}

step fmt-check
cargo +1.97 fmt --manifest-path "$rust_manifest" --all -- --check

step go-fmt-check
test -z "$(gofmt -l apps/solid-typefacts shims)"

step go-vet
go vet ./apps/solid-typefacts/...

step bun-install
bun install --cwd packages/cli --ignore-scripts --no-progress --frozen-lockfile

step compiler-identity
bun scripts/check-compiler-facts-identity.mjs

step build-typefacts
TYPEFACTS_BUILD_ID="${SOLID_CHECKER_BUILD_ID:-dev}" scripts/build-typefacts.sh

# ---------------------------------------------------------------------------
# The Makefile's CERTIFICATION_ENV, for every step below that builds.
#
# These four digests are read by `option_env!`, so they are part of the crate
# fingerprint and can only be supplied at compile time. Without them a build
# refuses Type Facts certification and probe authority *by design* — and a
# test binary built that way turns every probe-gate assertion into a silent
# early return, which is exactly what `make verify` used to do while reporting
# a green run. `SOLID_CHECKER_EXPECT_PROBE_PINS=1` turns that silence into a
# loud failure (see
# `probe_harness::tests::a_build_that_must_carry_probe_pins_carries_them`).
#
# Compute these before *every* Cargo build, including Clippy and feature
# checks, so preflight and tests do not alternate unpinned/pinned builds.
step certification-pins
if ! command -v node >/dev/null 2>&1; then
  echo "make verify: node is required (the certification pins compiled into the verifier are" >&2
  echo "  computed from the harness source manifest and the Node executable it launches)." >&2
  exit 127
fi
probe_node="${PROBE_NODE:-$(node -e 'process.stdout.write(require("fs").realpathSync(process.execPath))')}"
SOLID_TYPEFACTS_CERTIFICATION_SHA256="sha256:$(shasum -a 256 bin/solid-typefacts | awk '{print $1}')"
SOLID_TYPEFACTS_SOURCE_MANIFEST_SHA256="sha256:$(node scripts/typefacts-source-identity.mjs --build-id "${SOLID_CHECKER_BUILD_ID:-dev}" --digest)"
SOLID_CHECKER_PROBE_HARNESS_SHA256="sha256:$(node scripts/probe-harness-source-identity.mjs --build-id "${SOLID_CHECKER_BUILD_ID:-dev}" --write-stamp --digest)"
SOLID_CHECKER_PROBE_NODE_SHA256="sha256:$(shasum -a 256 "$probe_node" | awk '{print $1}')"
SOLID_CHECKER_EXPECT_PROBE_PINS=1
export SOLID_TYPEFACTS_CERTIFICATION_SHA256 SOLID_TYPEFACTS_SOURCE_MANIFEST_SHA256
export SOLID_CHECKER_PROBE_HARNESS_SHA256 SOLID_CHECKER_PROBE_NODE_SHA256
export SOLID_CHECKER_EXPECT_PROBE_PINS
# The resolved real path, not only its digest: every step below that resolves a
# Node executable of its own — the probe-gate tracers, and
# `scripts/check-bundled-contracts.mjs`, which recomputes these same pins for
# its `cargo run` — must use the executable whose bytes were just hashed
# rather than re-resolving `node` from `PATH`.
PROBE_NODE="$probe_node"
export PROBE_NODE
# ADR 0033: the optional browser pin. Only when the operator names a
# headless-shell executable; otherwise the browser profile refuses and its
# tracers skip, exactly as on a machine without one.
if [ -n "${PROBE_BROWSER:-}" ]; then
  SOLID_CHECKER_PROBE_BROWSER_SHA256="sha256:$(node scripts/probe-browser-identity.mjs "$PROBE_BROWSER")"
  SOLID_CHECKER_EXPECT_BROWSER_PIN=1
  export PROBE_BROWSER SOLID_CHECKER_PROBE_BROWSER_SHA256 SOLID_CHECKER_EXPECT_BROWSER_PIN
fi

SOLID_CHECKER_BUILD_ID="${SOLID_CHECKER_BUILD_ID:-dev}"
TYPEFACTS_BUILD_ID="$SOLID_CHECKER_BUILD_ID"
SOLID_CHECKER_CARGO_PROFILE="$cargo_profile"
export SOLID_CHECKER_BUILD_ID TYPEFACTS_BUILD_ID SOLID_CHECKER_CARGO_PROFILE

step clippy
cargo +1.97 clippy --profile "$cargo_profile" \
  --manifest-path "$rust_manifest" --workspace --all-targets -- -D warnings

step check-backend-v1
cargo +1.97 check --profile "$cargo_profile" \
  --manifest-path "$rust_manifest" -p solid-facts-backend \
  --all-targets --no-default-features --features dialect-v1

step check-backend-v2
cargo +1.97 check --profile "$cargo_profile" \
  --manifest-path "$rust_manifest" -p solid-facts-backend \
  --all-targets --no-default-features --features dialect-v2

step check-wasm-v1
cargo +1.97 check --profile "$cargo_profile" \
  --manifest-path "$rust_manifest" -p solid-checker-wasm \
  --all-targets --no-default-features --features dialect-v1

step check-wasm-v2
cargo +1.97 check --profile "$cargo_profile" \
  --manifest-path "$rust_manifest" -p solid-checker-wasm \
  --all-targets --no-default-features --features dialect-v2

# The product-owned corpus carries exact checker expectations and per-finding
# TypeScript ownership for every retained former parity case.
#
# Ahead of `test-workspace`, not after it, because the tree it installs is also
# `SOLID_CHECKER_RC3_ARCHIVE_ROOT` below. It is idempotent — `provision`
# short-circuits on a tree that already passes the version check — so the move
# costs nothing on a warm build root.
step oracle-provision
bun scripts/tsc-oracle.mjs provision --dialect all

# The rc.3 archives the 2.0 negative table's implementation-audited rows quote.
#
# Those rows cite byte ranges of `@solidjs/signals@2.0.0-rc.3`'s own runtime
# files, and the ranges are checked into
# `rust/crates/solid-dialect/audited-slices/` so the digests are verified with
# no install at all. This variable arms the *stronger* half: it re-reads the
# real archive and asserts the checked-in slice is still exactly
# `bytes[start..end]` of the pinned file — the one thing a checked-in copy
# cannot establish about itself. Absent the variable that arm skips, so
# `SOLID_CHECKER_EXPECT_PROBE_PINS=1` (set above) makes the skip a loud failure
# and this export is what keeps `make verify` from tripping it. The oracle tree
# is version-verified by `provision`, which refuses a substituted prerelease.
SOLID_CHECKER_RC3_ARCHIVE_ROOT="$PWD/rust/target/tsc-oracle/v2/node_modules"
export SOLID_CHECKER_RC3_ARCHIVE_ROOT

step go-rust-tests
TYPEFACTS_TEST_BIN="$PWD/bin/solid-typefacts" SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  node scripts/verify-tests.mjs "$rust_test_runner" &
tests_pid=$!
stop_tests() {
  kill -TERM "$tests_pid" 2>/dev/null || :
  wait "$tests_pid" 2>/dev/null || :
  exit 130
}
trap stop_tests INT TERM HUP
tests_status=0
wait "$tests_pid" || tests_status=$?
trap - INT TERM HUP
test "$tests_status" -eq 0

step build-checker
cargo +1.97 build --profile "$cargo_profile" --manifest-path "$rust_manifest" \
  -p solid-facts-backend --bin solid-checker-rust

step coverage
SOLID_CHECKER_BIN="$checker_bin" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/coverage.mjs

step ownership-gate
SOLID_CHECKER_BIN="$checker_bin" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/ownership-gate.mjs \
  --require-retained --require-complete

step build-session-bench
cargo +1.97 build --release --manifest-path "$rust_manifest" \
  -p solid-facts-backend --bin solid-checker-session-bench

# Absolute wall-time thresholds: this step must have the machine to itself, so
# it is deliberately the one place nothing else is scheduled alongside.
step verify-performance
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  bun benchmarks/verify-performance.mjs

step bun-test
bun run --cwd packages/cli test

# The ecosystem runner's own tests, among them the wall-time budget the pinned
# report has to meet. Until 2026-09-06 nothing here ran them: a pin taken at
# 224 s against the 150 s ceiling passed every gate below and failed only when
# the runner suite was run by hand. The budget is a host fact as much as a
# code fact — the same binary measures 85-105 s on mains and 170-200 s on
# battery in Low Power Mode — so a failure here is judged with `pmset -g` in
# hand (docs/precision-backlog.md, 2026-09-06).
step ecosystem-runner-test
bun packages/cli/node_modules/vitest/vitest.mjs run \
  --config packages/cli/vitest.config.mjs scripts/ecosystem-benchmark/*.test.mjs

# Phase 0 is immutable historical evidence. Validate its pinned bytes and
# internal invariants; never regenerate it from the current dependency graph.
step phase0-baseline
bun scripts/package-contract-v2-phase0.mjs --check

# Phase 16 pins full-corpus reachability/refusals and the accepted-corpus
# compactness/load/query measurements. Expensive ecosystem generation is a
# separately refreshed authority; this check refuses drift in any of its inputs.
step phase16-corpus
bun scripts/package-contract-v2-phase16.mjs --check

# Phase 18 proves that every live main producer, consumer, bundle, fixture,
# receipt, manifest, cache, and documentation path uses the first stable public
# schema version 1 without rewriting independent version namespaces.
step phase18-stable-cut
bun scripts/package-contract-phase18.mjs

# Phase 19A keeps policy 1 active while pinning the exact authority/refusal
# baseline and a Rust-owned, internal policy-2 manifest and digest. This gate
# fails if the audit rendering, producer identities, or issuance-shortcut
# inventory drifts before the atomic policy cut.
step phase19-policy
bun scripts/package-contract-phase19.mjs

# Phase 20 is the live row-disposition authority. Its generated ledger must
# remain byte-identical to the checked full-corpus report, including ordinary
# receipt loads, typed applicability, and complete dependency plans.
step phase20-ledger
bun scripts/package-contract-v2-phase20-ledger.mjs --check

# Phase 21 freezes the exact 30-row Phase 20 refusal cohort before tracking
# proof-preserving reductions. Its taxonomy must retain the upstream and CJS
# controls while keeping semantic terminal classes authoritative over prose.
step phase21-ledger
bun scripts/package-contract-v2-phase21-ledger.mjs --check

# Proposal orchestration can change a refusal envelope without changing the
# normalized Rust model. Keep the exact generator corpus in the local handoff
# authority as well as its dedicated CI workflow so snapshot drift cannot hide
# behind library and CLI unit coverage.
step contract-corpus
SOLID_CHECKER_NATIVE_BIN="$checker_bin" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  bun scripts/contract-corpus.mjs

# AGENTS.md's absolute rule, as a gate: no rule's positive case may also be a
# `tsc` error against the real published Solid typings. Provisioning installs
# the audited package versions and verifies them, so a drifted install fails
# here rather than changing the answer silently. The gate runs the checker over
# every case as well, so it takes the same fresh verification build as coverage
# and ownership -- the packaged binary may lag rust/ source.
step tsc-oracle-test
# The whole glob, exactly as CI's contracts job runs it: naming individual
# files here once let a contract-generation regression reach CI that every
# local handoff had missed, because verify gated 5 of the 17 test files.
SOLID_CHECKER_NATIVE_BIN="$checker_bin" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  bun packages/cli/node_modules/vitest/vitest.mjs run \
  --config packages/cli/vitest.config.mjs scripts/*.test.mjs

step tsc-oracle-gate
SOLID_CHECKER_BIN="$checker_bin" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/tsc-oracle-gate.mjs

# The other half: an *unreported* finding is a claim too. This supplies the
# evidence each obligation says is missing and asks whether the answer changes,
# so an over-conservatism cannot pass as a missing fact. It shares the oracle's
# provisioned installs for the same reason -- a loosened stub would invent the
# obligation it is meant to test.
step obligation-audit
SOLID_CHECKER_BIN="$checker_bin" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/obligation-audit.mjs

step lint-misc
sh -n scripts/*.sh
jq empty schema/*.json
jq empty fixtures/tsc-oracle/*.json
jq empty fixtures/obligation-cases/*.json
find pkg/contracts/bundled -type f -name '*.json' -exec jq empty {} +
bun scripts/dialect-manifests.mjs validate

step conformance
bun scripts/check-bundled-contracts.mjs
bun scripts/check-contract-pins.mjs
bun scripts/dialect-manifests.mjs check-composed-contracts

step ""
summarize
