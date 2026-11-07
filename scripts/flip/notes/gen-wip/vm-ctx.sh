#!/bin/zsh
# vm-ctx.sh <label> <contexts>: (one lock hold) census-ctx with bins/<label>; the VM clone's scripts are used as they are.
H=$HOME/EDF/tmp/flip-gen-await/vm; R=/mnt/lima-colima/sc/runs/gen-await; label=$1 contexts=$2; name=$label-ctx
colima ssh --profile sc -- bash -lc "cd /mnt/lima-colima/sc/solid-checker && rm -f $R/$name.done && (setsid nohup bash -c 'bash $H/guarded.sh $name bash $H/census-ctx.sh $label $R/bins/$label $contexts; touch $R/$name.done' >/dev/null 2>&1 < /dev/null &)"
until colima ssh --profile sc -- bash -lc "test -f $R/$name.done" 2>/dev/null; do sleep 15; done
colima ssh --profile sc -- bash -lc "cat $R/$name.status; grep -E 'rc=' $R/$name.log"
