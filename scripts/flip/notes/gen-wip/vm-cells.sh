#!/bin/zsh
# Under the lock: the VM clone is already at the fix (vm-plan); run plan-cells.sh detached and wait for it.
H=$HOME/EDF/tmp/flip-gen-await/vm
colima ssh --profile sc -- bash -lc "cd /mnt/lima-colima/sc/solid-checker && grep -q 'ADR 0850' rust/crates/solid-reactive-ir/src/static_rules/after_await.rs && setsid nohup bash $H/plan-cells.sh >/dev/null 2>&1 < /dev/null & echo launched" || exit 4
until colima ssh --profile sc -- bash -lc 'grep -q "cells done" /mnt/lima-colima/sc/runs/gen-await/plan.log 2>/dev/null'; do sleep 20; done
echo "cells done"
