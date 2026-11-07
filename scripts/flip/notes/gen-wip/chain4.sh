#!/bin/zsh
source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"
S=$HOME/EDF/tmp/flip-gen-await; cd $S/wt || exit 2
step() { local name=$1; shift; local t0=$(date +%s); $S/tools/withlock.sh $name "$@" > $S/$name.log 2>&1; local rc=$?; echo "$(date '+%F %T') $name rc=$rc $(( $(date +%s)-t0 ))s" | tee -a $S/gate-times.tsv; return $rc; }
step coverage-d1 make coverage
step ownership-gate-d1 make ownership-gate
echo chain4 done
