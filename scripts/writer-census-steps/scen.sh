#!/bin/sh
# scen.sh <scenarios.json> [names]: the scenario runner over the corpus (ONE sc-heavy step). Names are comma separated; none means the whole file.
set -u
HEAD="${HEAD:-/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0}"
S="${S:-/Users/thomas/dev/sc-scratch.noindex/mac20b}"
cd "$HEAD" || exit 2
echo "== scenarios $(date +%H:%M:%S) $1 ${2:-all}"
if [ -n "${2:-}" ]; then
  node scripts/writer-census-scenarios.mjs --baseline "$S/runs/census-v2" --out "$S/scen" --checker "$S/pin/checker-head" --typefacts "$S/pin/typefacts" --scenarios "$1" --only "$2" --jobs 4
else
  node scripts/writer-census-scenarios.mjs --baseline "$S/runs/census-v2" --out "$S/scen" --checker "$S/pin/checker-head" --typefacts "$S/pin/typefacts" --scenarios "$1" --jobs 4
fi
echo "== done $(date +%H:%M:%S)"
