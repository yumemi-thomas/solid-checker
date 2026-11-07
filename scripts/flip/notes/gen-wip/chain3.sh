#!/bin/zsh
source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"
S=$HOME/EDF/tmp/flip-gen-await; cd $S/wt || exit 2
step() { local name=$1; shift; local t0=$(date +%s); $S/tools/withlock.sh $name "$@" > $S/$name.log 2>&1; local rc=$?; echo "$(date '+%F %T') $name rc=$rc $(( $(date +%s)-t0 ))s" | tee -a $S/gate-times.tsv; return $rc; }
step verify-delta make verify-delta
step verify make verify
echo chain3 done
