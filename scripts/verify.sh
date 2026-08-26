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

run_rust_tests() {
  if [ "$rust_test_runner" = nextest ]; then
    cargo +1.97 nextest run --cargo-profile "$cargo_profile" "$@"
  else
    cargo +1.97 test --profile "$cargo_profile" "$@"
  fi
}

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
# previous step (printing its wall time) and opens the next, so the commands
# below stay exactly what they were -- same commands, same order, same
# fail-fast: nothing is wrapped, nothing is subshelled, `set -eu` still aborts
# the script on the first failure.
#
# The clock is a single `bun -e` per boundary (bun is already required by
# steps below), so one read serves as the previous step's end and the next
# step's start: 22 reads for 21 steps. `date` is not used because POSIX `date`
# has no sub-second field and several steps finish in tens of milliseconds.
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

step go-test-race
go test -race ./apps/solid-typefacts/...

step clippy
cargo +1.97 clippy --profile "$cargo_profile" \
  --manifest-path "$rust_manifest" --workspace --all-targets

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

step build-typefacts
scripts/build-typefacts.sh

step test-workspace
TYPEFACTS_TEST_BIN="$PWD/bin/solid-typefacts" SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  run_rust_tests --manifest-path "$rust_manifest" --workspace

step build-checker
cargo +1.97 build --profile "$cargo_profile" --manifest-path "$rust_manifest" \
  -p solid-facts-backend --bin solid-checker-rust

step coverage
SOLID_CHECKER_BIN="$checker_bin" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/coverage.mjs

step bun-install
bun install --cwd packages/cli --ignore-scripts --no-progress --frozen-lockfile

# The product-owned corpus carries exact checker expectations and per-finding
# TypeScript ownership for every retained former parity case.
step oracle-provision
bun scripts/tsc-oracle.mjs provision --dialect all

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
bun scripts/generate-solid1-runtime-surface.mjs --check
bun scripts/dialect-manifests.mjs check-composed-contracts

step ""
summarize
