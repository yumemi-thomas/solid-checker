#!/usr/bin/env bash
# census-codes.sh <label> <bins dir> <codes>: mx-census for those codes in every context, every row through uncached Chrome.
root=/mnt/lima-colima/sc; R=$root/runs/gen-await
label=$1 bins=$2 codes=$3
rm -rf "$R/$label-codes"
node scripts/fuzz/matrix.mjs --grid --census --seed 1 --codes "$codes" --out "$R/$label-codes" \
  --confirm-with-chrome --chrome-false-clean 1000 --chrome-sample 1000 --chrome-other 1000 --no-chrome-cache \
  --checker-bin "$bins/solid-checker-rust" --typefacts "$bins/solid-typefacts" \
  --fast-host chrome --chrome-pages reuse --chrome-clock virtual-time --executor rust --concurrency 12 --workers 12 --chrome-concurrency 12
echo "codes rc=$?"
