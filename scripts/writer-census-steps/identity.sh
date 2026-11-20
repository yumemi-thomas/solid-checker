#!/bin/sh
# The two sweeps of the byte-identity check: the base build (dba9d3cee) and the head build with the census and the what-if off, over the 34-project corpus.
# Run under sc-heavy as ONE step. Each keeps the checker's stdout whole (gzipped) and its sha256.
set -u
HEAD="${HEAD:-/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0}"
BASE="${BASE:-/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0-base}"
OUT="${OUT:-/Users/thomas/dev/sc-scratch.noindex/mac20b/runs}"
cd "$HEAD" || exit 2
echo "== base sweep $(date +%H:%M:%S)"
node scripts/writer-census.mjs --checker "$BASE/rust/target/release/solid-checker-rust" --typefacts "$BASE/bin/solid-typefacts" --out "$OUT/identity-base" --keep-report --jobs 4 || echo "base sweep failed"
echo "== head sweep, switch off $(date +%H:%M:%S)"
node scripts/writer-census.mjs --checker "$HEAD/rust/target/release/solid-checker-rust" --typefacts "$HEAD/bin/solid-typefacts" --out "$OUT/identity-head-off" --keep-report --jobs 4 || echo "head sweep failed"
echo "== done $(date +%H:%M:%S)"
