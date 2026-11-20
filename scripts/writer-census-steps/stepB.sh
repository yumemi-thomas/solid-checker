#!/bin/sh
# Step B, ONE sc-heavy call: the what-if build (ADR 0950), pinned, and the checks on it:
#   whatif-off   no switch: its reports differ from the base's by the `assumed` taint alone
#   whatif-spot  the switch, on the scenarios that carry the headline numbers, against the runs of the build before the gate
set -u
HEAD="${HEAD:-/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0}"
S="${S:-/Users/thomas/dev/sc-scratch.noindex/mac20b}"
cd "$HEAD" || exit 2
echo "== what-if build $(date +%H:%M:%S) $(git rev-parse --short HEAD)"
make build-what-if || { echo "what-if build failed"; exit 3; }
cp rust/target-what-if/release/solid-checker-what-if "$S/pin/checker-whatif"
echo "== whatif-off $(date +%H:%M:%S)"
node scripts/writer-census.mjs --checker "$S/pin/checker-whatif" --typefacts "$S/pin/typefacts" --out "$S/runs/whatif-off" --keep-report --jobs 4 2>&1 | tail -2
echo "== whatif-spot $(date +%H:%M:%S)"
node scripts/writer-census-scenarios.mjs --baseline "$S/runs/census-v2" --out "$S/scen-whatif" --checker "$S/pin/checker-whatif" --typefacts "$S/pin/typefacts" --scenarios "$S/tables/v2-pre/stageAB.json" --only floor,v2,M7v,M2b,whatif-M3u --jobs 4
echo "== done $(date +%H:%M:%S)"
