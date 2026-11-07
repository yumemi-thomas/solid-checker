#!/bin/zsh
# Each heavy job under its own lock hold: coverage, ownership-gate, new release build, VM plan, sweep old, sweep new.
source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"
S=$HOME/EDF/tmp/flip-gen-await; W=$S/wt; cd $W || exit 2
grep -q 'if !seen.insert(span)' rust/crates/solid-facts/src/ast/types.rs && git merge-base --is-ancestor e40429f5e HEAD || { echo REFUSE; exit 9; }
step() { local name=$1; shift; local t0=$(date +%s); $S/tools/withlock.sh $name "$@" > $S/$name.log 2>&1; local rc=$?; echo "$(date '+%F %T') $name rc=$rc $(( $(date +%s)-t0 ))s" | tee -a $S/gate-times.tsv; return $rc; }
step coverage make coverage
step ownership-gate make ownership-gate
step build-new zsh -c "make build-checker-release && mkdir -p $S/bins/new && cp rust/target/release/solid-checker-rust bin/solid-typefacts $S/bins/new/ && shasum -a 256 $S/bins/new/*"
step vm-plan $S/tools/vm-hold1.sh
for side in old new; do
  step sweep-$side node scripts/corpus-sweep.mjs --manifest scripts/corpus/rc21-manifest.json --apps-root ../../../Yumemi/GitHub/solid-checker-rc21-frozen/apps \
    --checker $S/bins/$side/solid-checker-rust --typefacts $S/bins/$side/solid-typefacts --jobs 4 --order-from $HOME/EDF/tmp/flipw/sweep-w2001.json --out $S/sweep-$side.json
done
echo chain done
