#!/bin/sh
# Pipeline 2, ONE sc-heavy step: every scenario of the plan on the corpus (the floor, V2, the class matcher, every mechanism alone, the plan's sets), then the second run of the
# base on app-game (its SC5003 findings change between runs of one build, so the identity check needs the base against itself).
set -u
HEAD="${HEAD:-/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0}"
S="${S:-/Users/thomas/dev/sc-scratch.noindex/mac20b}"
cd "$HEAD" || exit 2
echo "== scenarios $(date +%H:%M:%S)"
node scripts/writer-census-scenarios.mjs --baseline "$S/runs/census-v2" --out "$S/scen" --checker "$S/pin/checker-head" --typefacts "$S/pin/typefacts" --scenarios "$S/tables/v2-pre/stageAB.json" --jobs 4
echo "== base again, app-game $(date +%H:%M:%S)"
node scripts/writer-census.mjs --checker "$S/pin/checker-base" --typefacts "$S/pin/typefacts" --out "$S/runs/identity-base2" --keep-report --projects app-game/apps/web/tsconfig.json --jobs 1 2>&1 | tail -2
echo "== done $(date +%H:%M:%S)"
