#!/usr/bin/env bash
# ids.sh <label> <bins dir> <manifest> [extra…]: the UNTRACKED_READ_AFTER_AWAIT gen-yield/gen-sync-loop ids through matrix.mjs with uncached Chrome.
root=/mnt/lima-colima/sc; R=$root/runs/gen-await
label=$1 bins=$2 manifest=$3; shift 3
exec node scripts/fuzz/matrix.mjs --grid --recall --seed 1 --codes UNTRACKED_READ_AFTER_AWAIT --contexts gen-yield,gen-sync-loop --ids "$manifest" \
  --out "$R/$label" --confirm-with-chrome --runtime-diff all --chrome-false-clean 1000 --chrome-sample 0 --chrome-other 0 \
  --checker-bin "$bins/solid-checker-rust" --typefacts "$bins/solid-typefacts" \
  --fast-host chrome --chrome-pages reuse --chrome-clock virtual-time --executor rust --no-chrome-cache "$@"
