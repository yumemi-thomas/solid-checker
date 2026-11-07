#!/usr/bin/env bash
# In the VM: new ids, then old and new cells+lg, one after another (each guarded).
H=/Users/thomas.guehenneux/EDF/tmp/flip-gen-await/vm; B=/mnt/lima-colima/sc/runs/gen-await/bins
bash $H/guarded.sh new-70 bash $H/ids.sh new-70 $B/new $H/ids-70.json
bash $H/guarded.sh new-cells bash $H/cells.sh new $B/new
bash $H/guarded.sh old-cells bash $H/cells.sh old $B/old
echo "plan done $(date '+%F %T')" >> /mnt/lima-colima/sc/runs/gen-await/plan.log
