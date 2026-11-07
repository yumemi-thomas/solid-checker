#!/bin/zsh
# Under the lock: sync the VM clone to the fixed worktree, keep its binaries as bins/new, run plan-new.sh detached, wait for it.
source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"
cd $HOME/EDF/tmp/flip-gen-await/wt || exit 2
FUZZ_VM_PROFILE=sc scripts/fuzz/vm.sh sync || exit 3
H=$HOME/EDF/tmp/flip-gen-await/vm
colima ssh --profile sc -- bash -lc "set -e; root=/mnt/lima-colima/sc; c=\$root/solid-checker; cd \$c; git log -1 --format='%h %p %s'; grep -q 'if !seen.insert(span)' rust/crates/solid-facts/src/ast/types.rs; git merge-base --is-ancestor e40429f5e HEAD; grep -q 'ADR 0850' rust/crates/solid-reactive-ir/src/static_rules/after_await.rs; mkdir -p \$root/runs/gen-await/bins/new; cp rust/target/release/solid-checker-rust bin/solid-typefacts \$root/runs/gen-await/bins/new/; sha256sum \$root/runs/gen-await/bins/*/*; rm -f \$root/runs/gen-await/plan.log; setsid nohup bash $H/plan-new.sh >/dev/null 2>&1 < /dev/null & echo launched" || exit 4
until colima ssh --profile sc -- bash -lc 'grep -q "plan done" /mnt/lima-colima/sc/runs/gen-await/plan.log 2>/dev/null'; do sleep 20; done
echo "plan done"
