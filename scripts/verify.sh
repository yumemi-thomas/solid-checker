#!/bin/sh
set -eu

# What this run verifies, captured before any step: the green stamp written at
# the end must name exactly this commit, on a tree that was clean throughout.
verify_start_head=$(git rev-parse HEAD)
verify_start_status=$(git status --porcelain)
verify_tree_moved=0

rust_manifest=rust/Cargo.toml
# The edit loop's own profile (`make verify-delta`, `make test-focused`, bare
# `cargo test`), so a handoff after a delta loop reuses its incremental build
# instead of compiling the workspace a second time. Cargo writes the `dev`
# profile under `debug/`.
cargo_profile=dev
checker_bin="$PWD/rust/target/debug/solid-checker-rust"
rust_test_runner="${SOLID_CHECKER_RUST_TEST_RUNNER:-auto}"
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
# previous step (printing its wall time) and opens the next. A step that runs
# lanes side by side is timed as a whole, and each lane's own steps are listed
# under the table; `set -eu` prevents later steps from running if any lane
# fails.
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
  # A tree that differs from the start at any step boundary (an edit, even one
  # reverted before the end) means some step may have checked other bytes than
  # the commit, so the run must not stamp it green.
  if [ "$(git status --porcelain)" != "$verify_start_status" ]; then
    verify_tree_moved=1
  fi
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

# ---------------------------------------------------------------------------
# Lanes: independent sequences run side by side within one step.
#
# `lane <name> <function>` starts the function in a background subshell whose
# output goes to its own log; `lanes_wait` waits for every lane, prints each
# log whole (so the lanes never interleave) and fails the step if any lane
# failed. Inside a lane, `lane_step <name>` times the lane's own steps.
#
# The lanes of one step never run two Cargo processes on one target directory
# (AGENTS.md): at most one lane builds in `rust/target`, and at most one in
# `apps/solid-typefacts/target`.
# ---------------------------------------------------------------------------

lane_root=$(mktemp -d "${TMPDIR:-/tmp}/sc-verify-lanes.XXXXXX")
lane_pids=""
lane_names=""

lane() {
  lane_name=$1
  shift
  # Job control gives each lane a process group of its own (led by the lane's
  # subshell), so stopping a lane also stops what it started: cargo's rustc
  # children, a gate's checker processes.
  set -m
  (
    # `set -e` stays in force inside the function: it is called on its own,
    # not as the left side of `||`, which would switch it off.
    set -eu
    lane_current=""
    lane_started=$(epoch_ms)
    lane_step_started=$lane_started
    trap 'lane_status=$?; lane_step ""; printf "%s\n" "$lane_status" > "$lane_root/$lane_name.status"' EXIT
    "$@"
  ) < /dev/null > "$lane_root/$lane_name.log" 2>&1 &
  lane_pids="$lane_pids $!"
  set +m
  lane_names="$lane_names $lane_name"
}

# Called only inside a lane: closes the lane's previous step, opens the next.
lane_step() {
  if [ -n "$lane_current" ]; then
    lane_now=$(epoch_ms)
    lane_ms=$((lane_now - lane_step_started))
    printf '%s/%s %s\n' "$lane_name" "$lane_current" "$lane_ms" >> "$lane_root/timings"
    printf '=== %s: %s %d.%03ds\n' "$lane_name" "$lane_current" $((lane_ms / 1000)) $((lane_ms % 1000))
    lane_step_started=$lane_now
  fi
  lane_current="${1-}"
}

lanes_wait() {
  for pid in $lane_pids; do
    wait "$pid" || :
  done
  lane_pids=""
  lanes_failed=""
  for name in $lane_names; do
    printf '\n----- lane %s -----\n' "$name"
    cat "$lane_root/$name.log"
    if [ "$(cat "$lane_root/$name.status" 2>/dev/null || echo missing)" != 0 ]; then
      lanes_failed="$lanes_failed $name"
    fi
  done
  lane_names=""
  if [ -n "$lanes_failed" ]; then
    printf '\n=== lane(s) failed:%s\n' "$lanes_failed" >&2
    return 1
  fi
}

stop_lanes() {
  for pid in $lane_pids; do
    kill -TERM -- "-$pid" 2>/dev/null || :
  done
}

# A failure exits before its step is closed, so the step name is the one thing
# the timing machinery still owes the reader.
on_exit() {
  status=$?
  # A step failing while lanes still run must not orphan them.
  stop_lanes
  rm -rf "$lane_root"
  if [ "$status" -ne 0 ] && [ -n "$step_name" ]; then
    printf '\n=== FAILED during step %s (exit %d)\n' "$step_name" "$status" >&2
  fi
}
trap on_exit EXIT
trap 'exit 130' INT TERM HUP

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
  if [ -f "$lane_root/timings" ]; then
    printf '\n  within lanes (side by side, so these overlap):\n'
    awk '{ printf "    %-32s %9.2f\n", $1, $2 / 1000 }' "$lane_root/timings"
  fi
}

step fmt-check
cargo +1.99.1 fmt --manifest-path "$rust_manifest" --all -- --check

step typefacts-fmt-check
cargo +1.99.1 fmt --manifest-path apps/solid-typefacts/Cargo.toml -- --check

step bun-install
bun install --cwd packages/cli --ignore-scripts --no-progress --frozen-lockfile
bun install --cwd packages/wasm --ignore-scripts --no-progress --frozen-lockfile

step compiler-identity
bun scripts/check-compiler-facts-identity.mjs

# Always the release producer the handoff is measured and certified with. A
# thin-LTO inner-loop producer (`TYPEFACTS_PROFILE=bench-dev`) carries another
# stamp, so this replaces it.
step build-typefacts
env -u TYPEFACTS_PROFILE TYPEFACTS_BUILD_ID="${SOLID_CHECKER_BUILD_ID:-dev}" scripts/build-typefacts.sh

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

# The WASM package and its parity gate run unless the branch changes only the
# analysis crates (or prose), whose threads a lint confines to sites with a
# sequential arm (ADR 0379). CI runs parity on every pull request either way.
# SOLID_CHECKER_VERIFY_WASM=1 runs it regardless.
step wasm-scope
wasm_scope=$(bun scripts/verify-delta.mjs --wasm-scope)
case "$wasm_scope" in
  skip:*) run_wasm=0 ;;
  *) run_wasm=1 ;;
esac
if [ "${SOLID_CHECKER_VERIFY_WASM:-0}" = 1 ]; then
  run_wasm=1
  wasm_scope="run: SOLID_CHECKER_VERIFY_WASM=1"
fi
echo "make verify: WASM package and parity: $wasm_scope"

# ---------------------------------------------------------------------------
# Builds and suites: five lanes, one per target directory (plus provisioning,
# which runs no Cargo).
#
# One dialect ships (ADR 0110), so `--no-default-features --features dialect-v2`
# is the default build and a separate arm would compile the same configuration
# twice. These steps return when a second dialect does.
# ---------------------------------------------------------------------------

# The checker workspace's suite, the checker the gates run, and the release
# session bench verify-performance runs.
rust_lane() {
  lane_step rust-tests
  TYPEFACTS_TEST_BIN="$PWD/bin/solid-typefacts" SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
    node scripts/verify-tests.mjs "$rust_test_runner" test-workspace
  lane_step build-checker
  cargo +1.99.1 build --profile "$cargo_profile" --manifest-path "$rust_manifest" \
    -p solid-facts-backend --bin solid-checker-rust
  lane_step build-session-bench
  cargo +1.99.1 build --release --manifest-path "$rust_manifest" \
    -p solid-facts-backend --bin solid-checker-session-bench
}

# The Type Facts producer is its own Cargo workspace (tsrs optimised even in the
# dev profile, its own target directory): native and the wasm32-wasip1 reactor,
# then its suite.
typefacts_lane() {
  lane_step typefacts-clippy
  cargo +1.99.1 clippy --manifest-path apps/solid-typefacts/Cargo.toml --locked --all-targets -- -D warnings
  cargo +1.99.1 clippy --manifest-path apps/solid-typefacts/Cargo.toml --locked \
    --target wasm32-wasip1 --features reactor --lib -- -D warnings
  lane_step typefacts-test
  TYPEFACTS_TEST_BIN="$PWD/bin/solid-typefacts" SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
    node scripts/verify-tests.mjs "$rust_test_runner" typefacts-test
}

# Its own target directory, which `make verify-delta` shares, so it compiles
# beside the test build instead of after it.
clippy_lane() {
  lane_step clippy
  cargo +1.99.1 clippy --profile "$cargo_profile" --target-dir rust/target/lint \
    --manifest-path "$rust_manifest" --workspace --all-targets -- -D warnings
}

# The WASM package (P5 S2): its Node-API module and the owned check's two
# reactors, built one after the other with the pins exported above, in a target
# directory of their own; then the parity gate's path-semantics control guest.
wasm_build_lane() {
  lane_step build-wasm-package
  CARGO_TARGET_DIR="$PWD/rust/target/wasm-package" bun run --cwd packages/wasm build
  cargo +1.99.1 build --locked --manifest-path scripts/wasm-parity/guests/path-control/Cargo.toml \
    --target wasm32-wasip1 --release --target-dir rust/target/wasm-parity-guests
}

# The product-owned corpus carries exact checker expectations and per-finding
# TypeScript ownership for every retained former parity case. The oracle tree
# is the audited release, version- and integrity-verified by `provision`, which
# refuses a substituted prerelease; it is idempotent, short-circuiting on a
# tree that already passes that check.
oracle_provision_lane() {
  lane_step oracle-provision
  bun scripts/tsc-oracle.mjs provision --dialect all
}

step builds-and-suites
lane rust rust_lane
lane typefacts typefacts_lane
lane clippy clippy_lane
if [ "$run_wasm" = 1 ]; then
  lane wasm-build wasm_build_lane
fi
lane oracle-provision oracle_provision_lane
lanes_wait

# ---------------------------------------------------------------------------
# Gates: three lanes that run what the step above built. Only the parity lane
# runs Cargo (one bridge test in `rust/target`).
# ---------------------------------------------------------------------------

# AGENTS.md's absolute rule, as a gate: no rule's positive case may also be a
# `tsc` error against the real published Solid typings. Provisioning installs
# the audited package versions and verifies them, so a drifted install fails
# here rather than changing the answer silently. The gates run the checker over
# every case, so they take the same fresh build as coverage and ownership --
# the packaged binary may lag rust/ source.
#
# The other half: an *unreported* finding is a claim too. The obligation audit
# supplies the evidence each obligation says is missing and asks whether the
# answer changes, so an over-conservatism cannot pass as a missing fact. It
# shares the oracle's provisioned installs for the same reason -- a loosened
# stub would invent the obligation it is meant to test.
gates_lane() {
  lane_step coverage
  SOLID_CHECKER_BIN="$checker_bin" \
    SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/coverage.mjs
  lane_step ownership-gate
  SOLID_CHECKER_BIN="$checker_bin" \
    SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/ownership-gate.mjs \
    --require-retained --require-complete
  lane_step tsc-oracle-gate
  SOLID_CHECKER_BIN="$checker_bin" \
    SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/tsc-oracle-gate.mjs
  lane_step obligation-audit
  SOLID_CHECKER_BIN="$checker_bin" \
    SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/obligation-audit.mjs
}

# The packed owned check, installed in isolation, against this checker. The
# gate builds nothing itself and fails on any missing input; the bridge test
# records the update frames it replays, in the same scratch tree outside the
# repository. See scripts/wasm-parity/README.md.
wasm_parity_lane() {
  lane_step wasm-parity
  wasm_parity_scratch=$(mktemp -d "${TMPDIR:-/tmp}/sc-wasm-parity.XXXXXX")
  mkdir -p "$wasm_parity_scratch/n2-tmp"
  TMPDIR="$wasm_parity_scratch/n2-tmp" TYPEFACTS_BRIDGE_FRAMES="$wasm_parity_scratch/update-frames" \
    TYPEFACTS_TEST_BIN="$PWD/bin/solid-typefacts" \
    cargo +1.99.1 test --profile "$cargo_profile" --manifest-path "$rust_manifest" \
    -p typefacts --test bridge_process -- --exact bridged_answers_equal_process_answers
  SOLID_CHECKER_BIN="$checker_bin" SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
    bun scripts/wasm-parity.mjs --scratch "$wasm_parity_scratch" --update-frames "$wasm_parity_scratch/update-frames"
}

js_lane() {
  # The checker this run built: without it the CLI launcher falls back to
  # `make build-rust`, a second Cargo in `rust/target`.
  lane_step bun-test
  SOLID_CHECKER_NATIVE_BIN="$checker_bin" SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
    bun run --cwd packages/cli test
  # The whole glob, exactly as CI's scripts-and-cli job runs it.
  lane_step scripts-test
  SOLID_CHECKER_NATIVE_BIN="$checker_bin" \
    SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
    bun packages/cli/node_modules/vitest/vitest.mjs run \
    --config packages/cli/vitest.config.mjs scripts/*.test.mjs
}

step gates
lane gates gates_lane
if [ "$run_wasm" = 1 ]; then
  lane wasm-parity wasm_parity_lane
fi
lane js js_lane
lanes_wait

# Absolute wall-time thresholds: this step must have the machine to itself, so
# it is deliberately the one place nothing else is scheduled alongside.
step verify-performance
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  bun benchmarks/verify-performance.mjs

step lint-misc
sh -n scripts/*.sh
jq empty schema/*.json
jq empty fixtures/tsc-oracle/*.json
jq empty fixtures/obligation-cases/*.json
bun scripts/dialect-manifests.mjs validate
bun scripts/doc-budget.mjs

step ""
summarize
printf '\n  WASM package and parity: %s\n' "$wasm_scope"

# A clean tree that passed every step is the default base for the next
# `make verify-delta` (scripts/verify-delta.mjs): the fast loop then plans only
# what changed since the last full green run. Stamp only when the run started
# clean, HEAD never moved, and the tree matched the start at every step
# boundary. An edit made and reverted inside a single step is not observable
# here; parallel agents work in their own worktrees, which keeps that window
# to a deliberate act.
if [ -z "$verify_start_status" ] && [ "$verify_tree_moved" = 0 ] &&
  [ -z "$(git status --porcelain)" ] && [ "$(git rev-parse HEAD)" = "$verify_start_head" ]; then
  printf '%s\n' "$verify_start_head" > rust/target/verify-green-commit
fi
