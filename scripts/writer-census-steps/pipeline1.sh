#!/bin/sh
# Pipeline 1, ONE sc-heavy step: build the head release checker, pin it, then the three corpus sweeps of the byte-identity check and the census:
#   census-v2        head, census on (what the tables read; keeps stdout)
#   identity-base    base (dba9d3cee), nothing on
#   identity-head-off head, census and what-if off
set -u
HEAD="${HEAD:-/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0}"
BASE="${BASE:-/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0-base}"
S="${S:-/Users/thomas/dev/sc-scratch.noindex/mac20b}"
cd "$HEAD" || exit 2
echo "== build $(date +%H:%M:%S) $(git rev-parse --short HEAD)"
make build-checker-release || { echo "build failed"; exit 3; }
mkdir -p "$S/pin" && cp rust/target/release/solid-checker-rust "$S/pin/checker-head" && cp bin/solid-typefacts "$S/pin/typefacts" && cp "$BASE/rust/target/release/solid-checker-rust" "$S/pin/checker-base"
git rev-parse HEAD > "$S/pin/head.sha"
echo "== census-v2 $(date +%H:%M:%S)"
node scripts/writer-census.mjs --checker "$S/pin/checker-head" --typefacts "$S/pin/typefacts" --out "$S/runs/census-v2" --census --keep-report --jobs 4 2>&1 | tail -3
echo "== base $(date +%H:%M:%S)"
node scripts/writer-census.mjs --checker "$S/pin/checker-base" --typefacts "$S/pin/typefacts" --out "$S/runs/identity-base" --keep-report --jobs 4 2>&1 | tail -3
echo "== head, switch off $(date +%H:%M:%S)"
node scripts/writer-census.mjs --checker "$S/pin/checker-head" --typefacts "$S/pin/typefacts" --out "$S/runs/identity-head-off" --keep-report --jobs 4 2>&1 | tail -3
echo "== done $(date +%H:%M:%S)"
