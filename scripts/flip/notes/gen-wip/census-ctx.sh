#!/usr/bin/env bash
# census-ctx.sh <label> <bins dir> <contexts>: mx-census, all codes, those contexts, every row through uncached Chrome.
root=/mnt/lima-colima/sc; R=$root/runs/gen-await
label=$1 bins=$2 contexts=$3
rm -rf "$R/$label-ctx"
node scripts/fuzz/matrix.mjs --grid --census --seed 1 --contexts "$contexts" --out "$R/$label-ctx" \
  --confirm-with-chrome --chrome-false-clean 1000 --chrome-sample 1000 --chrome-other 1000 --no-chrome-cache \
  --checker-bin "$bins/solid-checker-rust" --typefacts "$bins/solid-typefacts" \
  --fast-host chrome --chrome-pages reuse --chrome-clock virtual-time --executor rust --concurrency 12 --workers 12 --chrome-concurrency 12
echo "ctx rc=$?"
