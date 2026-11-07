#!/bin/zsh
# vm-census.sh <label> <bins dir in VM>: run census.sh guarded, detached, and wait (call under the lock).
H=$HOME/EDF/tmp/flip-gen-await/vm; label=$1 bins=$2
colima ssh --profile sc -- bash -lc "cd /mnt/lima-colima/sc/solid-checker && rm -f /mnt/lima-colima/sc/runs/gen-await/$label.done && (setsid nohup bash -c 'bash $H/guarded.sh $label bash $H/census.sh $label $bins; touch /mnt/lima-colima/sc/runs/gen-await/$label.done' >/dev/null 2>&1 < /dev/null &) && echo launched" || exit 4
until colima ssh --profile sc -- bash -lc "test -f /mnt/lima-colima/sc/runs/gen-await/$label.done"; do sleep 15; done
colima ssh --profile sc -- bash -lc "cat /mnt/lima-colima/sc/runs/gen-await/$label.status; grep -E 'rc=' /mnt/lima-colima/sc/runs/gen-await/$label.log"
