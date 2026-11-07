#!/bin/zsh
# withlock.sh <tag> <command…>: the machine's one heavy-job lock (owner, 2026-11-07): mkdir "$(git rev-parse --git-common-dir)/sc-heavy.lock",
# retried every 30 s, removed when the job ends (trap, also on failure). Never nest.
L="$(git -C $HOME/EDF/tmp/flip-gen-await/wt rev-parse --path-format=absolute --git-common-dir)/sc-heavy.lock"
until mkdir "$L" 2>/dev/null; do sleep 30; done
trap 'rmdir "$L" 2>/dev/null' EXIT INT TERM HUP
echo "lock taken $(date '+%F %T') $1" >> $HOME/EDF/tmp/flip-gen-await/lock.log
tag=$1; shift
export CARGO_BUILD_JOBS=$(sysctl -n hw.perflevel0.physicalcpu)
"$@"; rc=$?
echo "lock released $(date '+%F %T') $tag rc=$rc" >> $HOME/EDF/tmp/flip-gen-await/lock.log
exit $rc
