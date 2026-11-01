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
#   scripts/fuzz/vm.sh detach <log> <command…>     sync, then run <command> in the VM clone detached from this shell (it survives
#                                       the SSH session, a host time limit and a sleeping host); output in <log> in the VM
#
# Environment: FUZZ_VM_PROFILE (the Colima profile, default `default`), FUZZ_VM_CPUS (default: the performance cores, so no vCPU
# lands on an efficiency core), FUZZ_VM_MEMORY (GiB, 32), FUZZ_VM_DISK (GiB, 150), FUZZ_VM_ROOT (the workspace in the VM, default
# /mnt/lima-colima/sc). Grid outputs and caches go on the VM's largest data disk: when the workspace sits on the small root disk
# (some Colima versions mount the data disk elsewhere, e.g. /var/lib/rancher), setup moves `runs/` and `tmp/` there and links them.
# Before every sync the VM's free bytes and inodes are checked: a grid writes a directory of files per program, and a full disk
# fails a run late with ENOSPC (seen as harness results and a hung Chrome).
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

# The workspace's runs/ and tmp/ on the VM's largest non-root filesystem when the workspace itself is on the root disk.
place_storage() {
  vm "set -euo pipefail
    root_dev=\$(df --output=source / | tail -1); here_dev=\$(df --output=source $root | tail -1)
    if [ \"\$root_dev\" = \"\$here_dev\" ]; then
      # A local block device's filesystem, not the host's virtiofs share of the home directory.
      data=\$(df --output=target,size,source,fstype -B1G | awk -v r=\"\$root_dev\" 'NR>1 && \$3!=r && \$3 ~ /^\\/dev\\// && \$4 ~ /^(ext4|xfs|btrfs)\$/ && \$2>=50 {print \$1; exit}')
      if [ -n \"\$data\" ]; then
        sudo mkdir -p \$data/sc-data && sudo chown \$(id -u):\$(id -g) \$data/sc-data
        for d in runs tmp; do
          if [ ! -L $root/\$d ]; then mkdir -p \$data/sc-data/\$d; [ -d $root/\$d ] && cp -a $root/\$d/. \$data/sc-data/\$d/ && rm -rf $root/\$d; ln -s \$data/sc-data/\$d $root/\$d; fi
        done
        echo \"vm.sh: runs/ and tmp/ on \$data (the data disk)\"
      else
        echo \"vm.sh: warning: $root is on the root disk and no data disk was found\" >&2
      fi
    fi"
}

# The per-program work directories (the checker's, the fast runtime's, Chrome's) on tmpfs. A grid creates and deletes a directory
# per program, about a million a full run: on the VM's disk each one is a write to a disk image in the host's home directory, which
# kept the host's fseventsd at a full core (2026-11-02). In RAM they cost the host nothing and never reach the image. Grid outputs and
# the runtime cache stay on the data disk. Idempotent: a mounted directory is left as it is; a reboot empties them, which loses nothing.
scratch_in_ram() {
  vm "set -euo pipefail
    for d in $clone/scripts/runtime-oracle-install/.solid-checker-fuzz $clone/scripts/runtime-oracle-install/.solid-checker-fuzz-chrome $clone/scripts/fuzz/install/.solid-checker-fuzz-fast; do
      mkdir -p \$d
      mountpoint -q \$d || { sudo mount -t tmpfs -o size=${FUZZ_VM_SCRATCH_GIB:-8}g,mode=0755,uid=\$(id -u),gid=\$(id -g) tmpfs \$d && echo \"vm.sh: \$d on tmpfs\"; }
    done"
}

# Refuse a run the VM's disk cannot hold: below 10 GiB or 10% of the inodes free where runs/ and tmp/ live.
preflight() {
  vm "set -euo pipefail; mkdir -p $root/runs $root/tmp
    for d in $root/runs/ $root/tmp/; do
      read -r avail ipct < <(df --output=avail,ipcent -B1G \$d | tail -1 | tr -d '%')
      [ \"\$avail\" -ge 10 ] && [ \"\$ipct\" -le 90 ] || { echo \"vm.sh: \$d has \$avail GiB and \$((100 - ipct))% of its inodes free; delete old runs (their cases/ directories hold most of the files)\" >&2; exit 2; }
    done"
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
  place_storage
  sync
}

# The checkout's state as a commit on HEAD: every file `git add -A` would take (tracked changes and new files, not ignored ones),
# written through a scratch index so the real index and the working tree are untouched, and kept reachable under refs/fuzz-vm/sync
# so the VM clone can fetch it.
sync() {
  start
  preflight
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
  scratch_in_ram
}

case "${1:-}" in
  setup) setup ;;
  sync) sync ;;
  run) shift; [ $# -gt 0 ] || die "run <command…>"; sync; vm "$vm_env; cd $clone && $(printf '%q ' "$@")" ;;
  shell) start; colima ssh --profile "$profile" -- bash -lc "$vm_env; cd $clone && exec bash -l" ;;
  detach) shift; [ $# -gt 1 ] || die "detach <log> <command…>"; log=$1; shift; sync
    vm "$vm_env; cd $clone && mkdir -p \$(dirname $(printf '%q' "$log")) && (setsid nohup $(printf '%q ' "$@") > $(printf '%q' "$log") 2>&1 < /dev/null &) ; echo vm.sh: detached, output in $(printf '%q' "$log")" ;;
  pull) [ $# -eq 3 ] || die "pull <vm path> <host dir>"; mkdir -p "$3"; vm "tar cf - -C $(printf '%q' "$2") ." | tar xf - -C "$3" ;;
  *) sed -n '2,28p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
