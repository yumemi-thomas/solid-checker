#!/usr/bin/env bash
# census.sh <label> <bins dir> [extra…]: mx-census, contexts gen-yield + gen-sync-loop, every row through uncached Chrome; then lg-recall gen-prefix + gen-drain.
root=/mnt/lima-colima/sc; R=$root/runs/gen-await
label=$1 bins=$2; shift 2
tools=(--checker-bin "$bins/solid-checker-rust" --typefacts "$bins/solid-typefacts")
host=(--fast-host chrome --chrome-pages reuse --chrome-clock virtual-time --executor rust --concurrency 12 --workers 12 --chrome-concurrency 12)
rm -rf "$R/$label-census" "$R/$label-lgtwin"
node scripts/fuzz/matrix.mjs --grid --census --seed 1 --contexts gen-yield,gen-sync-loop --out "$R/$label-census" \
  --confirm-with-chrome --chrome-false-clean 1000 --chrome-sample 1000 --chrome-other 1000 --no-chrome-cache "${tools[@]}" "${host[@]}" "$@"
echo "census rc=$?"
node scripts/fuzz/legal.mjs --grid --recall --seed 1 --contexts gen-prefix,gen-drain --out "$R/$label-lgtwin" --confirm-with-chrome --no-chrome-cache "${tools[@]}" "${host[@]}"
echo "lgtwin rc=$?"
