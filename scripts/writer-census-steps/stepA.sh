#!/bin/sh
# Step A, ONE sc-heavy call: the final commit's default build, pinned, and the identity checks on it:
#   final-off  the default build with nothing on, against the base (byte for byte)
#   final-env  the default build with the what-if switch set (floor drop lists and every class prefix): it must not read it, so its stdout is the base's
set -u
HEAD="${HEAD:-/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0}"
S="${S:-/Users/thomas/dev/sc-scratch.noindex/mac20b}"
cd "$HEAD" || exit 2
echo "== build $(date +%H:%M:%S) $(git rev-parse --short HEAD)"
make build-checker-release || { echo "build failed"; exit 3; }
cp rust/target/release/solid-checker-rust "$S/pin/checker-final" && git rev-parse HEAD > "$S/pin/final.sha"
echo "== final-off $(date +%H:%M:%S)"
node scripts/writer-census.mjs --checker "$S/pin/checker-final" --typefacts "$S/pin/typefacts" --out "$S/runs/final-off" --keep-report --jobs 4 2>&1 | tail -2
echo "== final-env $(date +%H:%M:%S)"
node scripts/writer-census-report.mjs drops --baseline "$S/runs/census-v2" --floor --out "$S/drops-floor" 2>&1 | tail -1
node scripts/writer-census.mjs --checker "$S/pin/checker-final" --typefacts "$S/pin/typefacts" --out "$S/runs/final-env" --keep-report --drop-dir "$S/drops-floor" --whatif-causes "escape:,generated:,prototype-,unread:,unloaded" --jobs 4 2>&1 | tail -2
echo "== done $(date +%H:%M:%S)"
