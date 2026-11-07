#!/usr/bin/env bash
H=/Users/thomas.guehenneux/EDF/tmp/flip-gen-await/vm; B=/mnt/lima-colima/sc/runs/gen-await/bins; R=/mnt/lima-colima/sc/runs/gen-await
rm -rf $R/new-cells $R/new-lg $R/old-cells $R/old-lg
bash $H/guarded.sh new-cells bash $H/cells.sh new $B/new
bash $H/guarded.sh old-cells bash $H/cells.sh old $B/old
echo "cells done $(date '+%F %T')" >> $R/plan.log
