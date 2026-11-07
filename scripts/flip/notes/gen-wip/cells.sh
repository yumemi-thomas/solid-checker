#!/usr/bin/env bash
# cells.sh <label> <bins dir>: both whole cells (gen-yield, gen-sync-loop) of mx-recall UNTRACKED_READ_AFTER_AWAIT, and lg-recall's whole code, Chrome on problems.
root=/mnt/lima-colima/sc; R=$root/runs/gen-await
label=$1 bins=$2
tools=(--checker-bin "$bins/solid-checker-rust" --typefacts "$bins/solid-typefacts")
host=(--fast-host chrome --chrome-pages reuse --chrome-clock virtual-time --executor rust --concurrency 12 --workers 12 --chrome-concurrency 12)
node scripts/fuzz/matrix.mjs --grid --recall --seed 1 --codes UNTRACKED_READ_AFTER_AWAIT --contexts gen-yield,gen-sync-loop --out "$R/$label-cells" \
  --confirm-with-chrome --chrome-false-clean 1000 --chrome-sample 0 --chrome-other 0 --no-chrome-cache "${tools[@]}" "${host[@]}"
echo "cells rc=$?"
node scripts/fuzz/legal.mjs --grid --recall --seed 1 --codes UNTRACKED_READ_AFTER_AWAIT --out "$R/$label-lg" --confirm-with-chrome --no-chrome-cache "${tools[@]}" "${host[@]}"
echo "lg rc=$?"
