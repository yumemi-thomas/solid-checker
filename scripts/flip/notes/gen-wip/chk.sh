#!/bin/zsh
# chk.sh <checker bin> <tsconfig> [flags…]: one-shot checker run, killed (its own PID tree only) above 12 GB RSS summed.
bin=$1; shift
wt=$HOME/EDF/tmp/flip-gen-await/wt
SOLID_CHECKER_DAEMON=0 SOLID_TYPEFACTS_BIN=$wt/bin/solid-typefacts "$bin" --format json --project "$@" &
pid=$!
while kill -0 $pid 2>/dev/null; do
  kb=$(ps -o rss= -p $pid $(pgrep -P $pid) 2>/dev/null | awk '{s+=$1} END {print s+0}')
  if (( kb > 12582912 )); then echo "GUARD: killed $pid at ${kb} KB" >&2; pkill -TERM -P $pid; kill -TERM $pid; sleep 2; kill -KILL $pid 2>/dev/null; exit 99; fi
  sleep 0.3
done
wait $pid
