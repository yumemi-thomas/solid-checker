#!/usr/bin/env bash
# One focus run of the trial: shard 0/40 of STORE_WRITE_OUTSIDE_SETTER's mx-recall, caches in ~/fuzz-cache.
set -uo pipefail
bins="$RUNNER_TEMP/bins"
out="$RUNNER_TEMP/fuzz-out"
# Peak memory of the whole machine, and the running grid's last log line, every 2 minutes.
( peak=0; while sleep 10; do used=$(free -m | awk '/^Mem:/{print $3}'); [ "$used" -gt "$peak" ] && peak=$used && echo "$peak" > "$RUNNER_TEMP/mem-peak"; done ) &
( while sleep 120; do f=$(ls -t "$out"/*/*.log "$out"/*/*/*.log 2>/dev/null | head -1); [ -n "$f" ] && echo "[progress] $(tail -1 "$f" | cut -c1-200)"; done ) &
start=$(date +%s)
status=0
/usr/bin/time -v -o "$RUNNER_TEMP/time.txt" node scripts/fuzz/focus.mjs --codes STORE_WRITE_OUTSIDE_SETTER --grids mx-recall --shard 0/40 \
  --split-codes --prune-cases --grid-timeout 110 --out "$out" \
  --checker-bin "$bins/solid-checker-rust" --typefacts "$bins/solid-typefacts" -- --cache-dir "$HOME/fuzz-cache" || status=$?
echo "focus run: $(( $(date +%s) - start )) s, exit $status"
grep -E "Maximum resident|Elapsed" "$RUNNER_TEMP/time.txt"
echo "machine memory peak: $(cat "$RUNNER_TEMP/mem-peak" 2>/dev/null) MiB used"
for f in "$out"/*/*/cache.json; do echo "$f"; node -e 'const c=require(process.argv[1]); console.log(`  runtime hits ${c.hits} misses ${c.misses}; typecheck hits ${c.typecheck?.hits} misses ${c.typecheck?.misses}`)' "$f"; done
exit $status
