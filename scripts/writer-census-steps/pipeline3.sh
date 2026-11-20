#!/bin/sh
# Pipeline 3, ONE sc-heavy step: the byte-identity check on programs the corpus does not hold: the textbook and the fixtures (monotone M2: any (site, code) that moves between the
# base and the head with the census and the what-if off fails), and the textbook with a no-op ablation (monotone M1: the head's findings byte-identical to the base's).
set -u
HEAD="${HEAD:-/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0}"
S="${S:-/Users/thomas/dev/sc-scratch.noindex/mac20b}"
cd "$HEAD" || exit 2
echo "== textbook, M1 identity $(date +%H:%M:%S)"
node scripts/monotone.mjs --base "$S/pin/checker-base" --head "$S/pin/checker-head" --typefacts "$S/pin/typefacts" --ablate-arm m0-noop --set textbook --concurrency 4 --out "$S/runs/monotone-textbook-m1.json"
echo "== textbook + fixtures, M2 $(date +%H:%M:%S)"
node scripts/monotone.mjs --base "$S/pin/checker-base" --head "$S/pin/checker-head" --typefacts "$S/pin/typefacts" --set textbook --set fixtures --concurrency 4 --out "$S/runs/monotone-fixtures-m2.json"
echo "== done $(date +%H:%M:%S)"
