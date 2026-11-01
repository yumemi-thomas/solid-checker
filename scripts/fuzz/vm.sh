#!/usr/bin/env bash
# The fuzzer in a Linux VM (Colima, Apple's Virtualization framework). On a Mac whose endpoint-security agents gate every process
# start, each fuzzed program pays for the checker's and the producer's starts: on 2026-10-30 one such Mac started at most ~127
# processes a second and checked ~12 programs a second; in the VM a start is free and the same recall grid ran 3.7x faster than
# the best macOS setting, every outcome equal (scripts/fuzz/README.md, The Linux VM). Elsewhere it buys nothing.
#
#   scripts/fuzz/vm.sh setup            create or start the VM; toolchains, a clone of this checkout, the builds, installs, Chrome
#   scripts/fuzz/vm.sh sync             the VM clone at this checkout as it is (HEAD, its changes, its new files); rebuild what moved
#   scripts/fuzz/vm.sh run <command…>   sync, then run <command> in the VM clone (e.g. node scripts/fuzz/focus.mjs --codes A …)
#   scripts/fuzz/vm.sh shell            a login shell in the VM clone, with the toolchains on PATH
#   scripts/fuzz/vm.sh pull <vm path> <host dir>   copy a run's output out of the VM
#
# Environment: FUZZ_VM_PROFILE (the Colima profile, default `default`), FUZZ_VM_CPUS (default: the performance cores, so no vCPU
# lands on an efficiency core), FUZZ_VM_MEMORY (GiB, 32), FUZZ_VM_DISK (GiB, 150), FUZZ_VM_ROOT (the workspace in the VM, default
# /mnt/lima-colima/sc: Colima's data disk, which takes FUZZ_VM_DISK; the VM's root disk is small).
# The toolchains are the repository's pins: Rust from the Makefile's RUST_TOOLCHAIN, bun from packages/cli's packageManager, tsrs
# at apps/solid-typefacts's revision, Chrome as the pinned Playwright names it (scripts/playwright-browser.mjs); Node is the host's
# version. The checkout must be under the home directory, which Colima mounts at the same path.
set -euo pipefail

repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
profile=${FUZZ_VM_PROFILE:-default}
cpus=${FUZZ_VM_CPUS:-$(sysctl -n hw.perflevel0.physicalcpu 2>/dev/null || sysctl -n hw.ncpu)}
memory=${FUZZ_VM_MEMORY:-32}
disk=${FUZZ_VM_DISK:-150}
root=${FUZZ_VM_ROOT:-/mnt/lima-colima/sc}
clone=$root/solid-checker
rust=$(sed -n 's/^RUST_TOOLCHAIN ?= *//p' "$repo/Makefile")
bun=$(sed -n 's/.*"packageManager": *"bun@\([^"]*\)".*/\1/p' "$repo/packages/cli/package.json")
tsrs_rev=$(sed -n 's/^tsrs_core = .*rev = "\([0-9a-f]*\)".*/\1/p' "$repo/apps/solid-typefacts/Cargo.toml")
node_version=${FUZZ_VM_NODE:-$(node --version 2>/dev/null | sed 's/^v//')}

die() { echo "vm.sh: $*" >&2; exit 2; }
command -v colima >/dev/null || die "colima is not installed (brew install colima docker)"
case "$repo" in "$HOME"/*) ;; *) die "the checkout ($repo) is not under $HOME, which Colima mounts";; esac
[ -n "$rust" ] && [ -n "$bun" ] && [ -n "$tsrs_rev" ] && [ -n "$node_version" ] || die "could not read the pins (rust '$rust', bun '$bun', tsrs '$tsrs_rev', node '$node_version')"

vm() { colima ssh --profile "$profile" -- bash -lc "$1"; }
# The VM's PATH and the fuzzer's environment: the pinned tools, scratch on the VM's own disk.
vm_env="export PATH=$root/node/bin:\$HOME/.bun/bin:\$HOME/.cargo/bin:\$PATH SOLID_CHECKER_TSRS_BIN=$root/tsrs/target/release/tsrs TMPDIR=$root/tmp; mkdir -p $root/tmp"

start() {
  if ! colima status --profile "$profile" >/dev/null 2>&1; then
    colima start --profile "$profile" --cpu "$cpus" --memory "$memory" --disk "$disk" --vm-type vz --mount-type virtiofs
  fi
}

setup() {
  start
  vm "set -euo pipefail
    sudo mkdir -p $root && sudo chown \$(id -u):\$(id -g) $root
    sudo apt-get update -qq >/dev/null
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential pkg-config git curl unzip xz-utils python3 ca-certificates libssl-dev cmake clang >/dev/null
    command -v rustup >/dev/null || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain $rust >/dev/null
    . \$HOME/.cargo/env && rustup toolchain install $rust --profile minimal >/dev/null
    if [ \"\$($root/node/bin/node --version 2>/dev/null)\" != v$node_version ]; then
      rm -rf $root/node && mkdir -p $root/node
      curl -sSfL https://nodejs.org/dist/v$node_version/node-v$node_version-linux-\$(uname -m | sed 's/aarch64/arm64/;s/x86_64/x64/').tar.xz | tar xJ -C $root/node --strip-components 1
    fi
    [ \"\$(\$HOME/.bun/bin/bun --version 2>/dev/null)\" = $bun ] || curl -fsSL https://bun.sh/install | bash -s bun-v$bun >/dev/null
    if [ ! -x $root/tsrs/target/release/tsrs ] || [ \"\$(git -C $root/tsrs rev-parse HEAD 2>/dev/null)\" != $tsrs_rev ]; then
      [ -d $root/tsrs/.git ] || { rm -rf $root/tsrs; git clone -q https://github.com/yumemi-thomas/tsrs $root/tsrs; }
      git -C $root/tsrs fetch -q origin $tsrs_rev && git -C $root/tsrs checkout -q --detach $tsrs_rev
      (cd $root/tsrs && cargo +$rust build --release -q -p tsrs_cli)
    fi
    [ -d $clone/.git ] || git clone -q --no-checkout $repo $clone"
  sync
}

# The checkout's state as a commit on HEAD: every file `git add -A` would take (tracked changes and new files, not ignored ones),
# written through a scratch index so the real index and the working tree are untouched, and kept reachable under refs/fuzz-vm/sync
# so the VM clone can fetch it.
sync() {
  start
  local index tree ref
  index=$(mktemp)
  cp "$(git -C "$repo" rev-parse --absolute-git-dir)/index" "$index"
  GIT_INDEX_FILE=$index git -C "$repo" add -A
  tree=$(GIT_INDEX_FILE=$index git -C "$repo" write-tree)
  rm -f "$index"
  ref=$(git -C "$repo" commit-tree "$tree" -p HEAD -m "vm.sh sync of $(git -C "$repo" rev-parse --short HEAD)")
  git -C "$repo" update-ref refs/fuzz-vm/sync "$ref"
  vm "set -euo pipefail; $vm_env
    cd $clone && git -c safe.directory='*' fetch -q $repo refs/fuzz-vm/sync && git checkout -q --detach --force FETCH_HEAD
    echo \"vm.sh: VM clone at \$(git log --oneline -1 HEAD)\"
    make -s build-checker-release >/dev/null
    for dir in scripts/fuzz/install scripts/runtime-oracle-install; do
      stamp=\$dir/node_modules/.vm-lock; sum=\$(sha256sum \$dir/package-lock.json | cut -c1-64)
      [ \"\$(cat \$stamp 2>/dev/null)\" = \"\$sum\" ] || { (cd \$dir && npm ci --silent) && echo \"\$sum\" > \$stamp; }
    done
    stamp=packages/cli/node_modules/.vm-lock; sum=\$(sha256sum packages/cli/bun.lock | cut -c1-64)
    [ \"\$(cat \$stamp 2>/dev/null)\" = \"\$sum\" ] || { (cd packages/cli && bun install --frozen-lockfile --silent) && echo \"\$sum\" > \$stamp; }
    node -e 'import(\"./scripts/playwright-browser.mjs\").then((m) => m.pinnedBrowser())' 2>/dev/null || {
      (cd scripts/runtime-oracle-install && ./node_modules/.bin/playwright install chromium-headless-shell >/dev/null && sudo env PATH=\$PATH ./node_modules/.bin/playwright install-deps chromium-headless-shell >/dev/null)
    }"
}

case "${1:-}" in
  setup) setup ;;
  sync) sync ;;
  run) shift; [ $# -gt 0 ] || die "run <command…>"; sync; vm "$vm_env; cd $clone && $(printf '%q ' "$@")" ;;
  shell) start; colima ssh --profile "$profile" -- bash -lc "$vm_env; cd $clone && exec bash -l" ;;
  pull) [ $# -eq 3 ] || die "pull <vm path> <host dir>"; mkdir -p "$3"; vm "tar cf - -C $(printf '%q' "$2") ." | tar xf - -C "$3" ;;
  *) sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
