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
  echo "make verify: bun is required (the per-step clock, the coverage/oracle gates," >&2
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
# gate caches (coverage, registry pins) to recompute
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
# Compile-time inputs and the Node the runtime resolver runs.
#
# Every Cargo build below shares one environment (build id, probe Node), so
# preflight and tests do not alternate between differently-fingerprinted builds.
step certification-pins
if ! command -v node >/dev/null 2>&1; then
  echo "make verify: node is required (the runtime resolver runs project config with it)." >&2
  exit 127
fi
probe_node="${PROBE_NODE:-$(node -e 'process.stdout.write(require("fs").realpathSync(process.execPath))')}"
# The runtime resolver (ADR 0220) runs project config with this Node.
PROBE_NODE="$probe_node"
SOLID_CHECKER_PROBE_NODE="$probe_node"
export PROBE_NODE SOLID_CHECKER_PROBE_NODE

SOLID_CHECKER_BUILD_ID="${SOLID_CHECKER_BUILD_ID:-dev}"
TYPEFACTS_BUILD_ID="$SOLID_CHECKER_BUILD_ID"
SOLID_CHECKER_CARGO_PROFILE="$cargo_profile"
export SOLID_CHECKER_BUILD_ID TYPEFACTS_BUILD_ID SOLID_CHECKER_CARGO_PROFILE

step clippy
cargo +1.97 clippy --profile "$cargo_profile" \
  --manifest-path "$rust_manifest" --workspace --all-targets -- -D warnings

# One dialect ships (ADR 0110), so `--no-default-features --features dialect-v2`
# is the default build and a separate arm would compile the same configuration
# twice. These steps return when a second dialect does.

# The product-owned corpus carries exact checker expectations and per-finding
# TypeScript ownership for every retained former parity case. The oracle tree
# is the audited release, version- and integrity-verified by `provision`, which
# refuses a substituted prerelease; it is idempotent, short-circuiting on a
# tree that already passes that check.
step oracle-provision
bun scripts/tsc-oracle.mjs provision --dialect all

# The assignments are inside the function rather than prefixed onto the call:
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

# The WASM package (P5 S2): its Node-API module and the owned check's two
# reactors, built one after the other with the pins exported above. Then the
# packed owned check, installed in isolation, against this checker. The gate builds nothing itself and fails on any
# missing input; the bridge test records the update frames it replays, in the
# same scratch tree outside the repository. See scripts/wasm-parity/README.md.
step build-wasm-package
bun install --cwd packages/wasm --ignore-scripts --no-progress --frozen-lockfile
bun run --cwd packages/wasm build
cargo +1.97 build --locked --manifest-path scripts/wasm-parity/guests/path-control/Cargo.toml \
  --target wasm32-wasip1 --release --target-dir rust/target/wasm-parity-guests
GOOS=wasip1 GOARCH=wasm GOTOOLCHAIN=local go build -trimpath \
  -o rust/target/wasm-parity-guests/path-control-go.wasm ./scripts/wasm-parity/guests/path-control-go

step wasm-parity
wasm_parity_scratch=$(mktemp -d "${TMPDIR:-/tmp}/sc-wasm-parity.XXXXXX")
mkdir -p "$wasm_parity_scratch/n2-tmp"
TMPDIR="$wasm_parity_scratch/n2-tmp" TYPEFACTS_BRIDGE_FRAMES="$wasm_parity_scratch/update-frames" \
  TYPEFACTS_TEST_BIN="$PWD/bin/solid-typefacts" \
  cargo +1.97 test --profile "$cargo_profile" --manifest-path "$rust_manifest" \
  -p typefacts --test bridge_process -- --exact bridged_answers_equal_process_answers
SOLID_CHECKER_BIN="$checker_bin" SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  bun scripts/wasm-parity.mjs --scratch "$wasm_parity_scratch" --update-frames "$wasm_parity_scratch/update-frames"

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
# The whole glob, exactly as CI's scripts-and-cli job runs it.
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
bun scripts/dialect-manifests.mjs validate
bun scripts/doc-budget.mjs

step ""
summarize

# A clean tree that passed every step is the default base for the next
# `make verify-delta` (scripts/verify-delta.mjs): the fast loop then plans only
# what changed since the last full green run.
if [ -z "$(git status --porcelain)" ]; then
  git rev-parse HEAD > rust/target/verify-green-commit
fi
