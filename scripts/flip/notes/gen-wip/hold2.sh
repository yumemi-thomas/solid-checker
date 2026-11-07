#!/bin/zsh
# Under the lock: release builds of base-wt (e57cc0dc5) and wt (the fix), then a corpus sweep with each.
source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"
S=$HOME/EDF/tmp/flip-gen-await
export CARGO_BUILD_JOBS=6
# Coordinator: start a heavy step only with load under 30.
# And one heavy job at a time with the casts agent: no other cargo or make verify running.
busy() { pgrep -x cargo >/dev/null || pgrep -f 'scripts/verify.sh' >/dev/null || (( $(sysctl -n vm.loadavg | awk '{print int($2)}') >= 30 )); }
waitload() { while busy; do sleep 15; done; echo "start $(date +%T) load=$(sysctl -n vm.loadavg)"; }
for side in old new; do
  dir=$S/wt; [ $side = old ] && dir=$S/base-wt
  cd $dir || exit 2
  grep -q 'if !seen.insert(span)' rust/crates/solid-facts/src/ast/types.rs && git merge-base --is-ancestor e40429f5e HEAD || { echo "REFUSE $side"; exit 9; }
  waitload; make build-checker-release > $S/build-release-$side.log 2>&1 || { echo "build $side failed"; exit 3; }
  mkdir -p $S/bins/$side && cp rust/target/release/solid-checker-rust bin/solid-typefacts $S/bins/$side/ && shasum -a 256 $S/bins/$side/*
done
cd $S/wt
for side in old new; do
  waitload
  /usr/bin/time -p node scripts/corpus-sweep.mjs --manifest scripts/corpus/rc21-manifest.json --apps-root ../../../Yumemi/GitHub/solid-checker-rc21-frozen/apps \
    --checker $S/bins/$side/solid-checker-rust --typefacts $S/bins/$side/solid-typefacts --jobs 4 --order-from $HOME/EDF/tmp/flipw/sweep-w2001.json \
    --out $S/sweep-$side.json > $S/sweep-$side.log 2>&1; echo "sweep $side rc=$?"
done
