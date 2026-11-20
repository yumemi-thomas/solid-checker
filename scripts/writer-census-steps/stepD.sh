#!/bin/sh
# Step D, ONE sc-heavy call: the design report's own columns (A, A+esc, B, B+esc) as runs, the production-build readings of M2, and the production-build census.
set -u
HEAD="${HEAD:-/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0}"
S="${S:-/Users/thomas/dev/sc-scratch.noindex/mac20b}"
cd "$HEAD" || exit 2
echo "== scenarios C $(date +%H:%M:%S)"
node scripts/writer-census-scenarios.mjs --baseline "$S/runs/census-v2" --out "$S/scen" --checker "$S/pin/checker-head" --typefacts "$S/pin/typefacts" --scenarios "$S/tables/v2-pre/stageC.json" --jobs 4
echo "== census, production build $(date +%H:%M:%S)"
node scripts/writer-census.mjs --checker "$S/pin/checker-head" --typefacts "$S/pin/typefacts" --out "$S/runs/census-std" --census --runtime-build production --jobs 4 2>&1 | tail -3
echo "== done $(date +%H:%M:%S)"
