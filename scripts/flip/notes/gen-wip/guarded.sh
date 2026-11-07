#!/usr/bin/env bash
# guarded.sh <name> <command…>: run <command> in the VM clone, in its own session. A solid-checker*/solid-typefacts process of its group above
# 12 GB RSS is killed alone (RUNAWAY); the whole group is stopped when `free -m` used passes 85% of total (TRIPPED).
set -u
root=/mnt/lima-colima/sc
clone=$root/solid-checker
R=$root/runs/gen-await
total=$(free -m | awk 'NR==2{print $2}'); LIMIT_MB=$(( total * 85 / 100 )); RUN_KB=$(( 12 * 1024 * 1024 ))
export PATH=$root/node/bin:$HOME/.bun/bin:$HOME/.cargo/bin:$PATH SOLID_CHECKER_TSRS_BIN=$root/tsrs/target/release/tsrs TMPDIR=$root/ram/tmp SOLID_CHECKER_FUZZ_SCRATCH=$root/ram/scratch
mkdir -p $root/ram/tmp $root/ram/scratch "$R"
name=$1; shift
cd "$clone" || exit 2
echo "start $(date '+%F %T') head=$(git rev-parse HEAD) args=$*" > "$R/$name.status"
setsid "$@" > "$R/$name.log" 2>&1 < /dev/null &
pid=$!
sleep 1
pgid=$(ps -o pgid= -p "$pid" | tr -d ' ')
[ "$pgid" = "$pid" ] || { echo "pgid differs" >> "$R/$name.status"; kill -TERM "$pid"; exit 2; }
tripped=0
while kill -0 "$pid" 2>/dev/null; do
  used=$(free -m | awk 'NR==2{print $3}')
  while read -r p g rss comm; do
    if [ "$g" = "$pgid" ] && [ "$rss" -gt "$RUN_KB" ]; then echo "RUNAWAY $(date '+%F %T') pid=$p comm=$comm rss_kb=$rss" >> "$R/$name.status"; kill -KILL "$p"; fi
  done < <(ps -eo pid=,pgid=,rss=,comm= | awk '$4 ~ /^solid-(checker|typefact)/')
  echo "$(date +%T) used=$used" >> "$R/$name.mem"
  if [ "$used" -gt "$LIMIT_MB" ]; then echo "TRIPPED $(date '+%F %T') used=$used" >> "$R/$name.status"; kill -TERM -- "-$pgid"; sleep 5; kill -KILL -- "-$pgid" 2>/dev/null; tripped=1; break; fi
  sleep 5
done
wait "$pid"; code=$?
echo "end $(date '+%F %T') exit=$code tripped=$tripped" >> "$R/$name.status"
