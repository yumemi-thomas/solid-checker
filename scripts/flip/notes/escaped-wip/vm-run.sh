#!/usr/bin/env bash
# Runs inside the sc VM (colima ssh). Own dirs only: $R/carrier-{old,new} (copies of the host worktrees), $R/runs/carrier-<tag>.
# vm-run.sh <tag> : build both if their source moved, then run the id manifests with each checker; the generator is the new tree's.
set -uo pipefail
R=/mnt/lima-colima/sc
H=/Users/thomas.guehenneux/EDF/tmp/flipcarrier
tag=${1:?tag}
export PATH=$R/node/bin:$HOME/.bun/bin:$HOME/.cargo/bin:$PATH SOLID_CHECKER_TSRS_BIN=$R/tsrs/target/release/tsrs TMPDIR=$R/ram/carrier-tmp SOLID_CHECKER_FUZZ_SCRATCH=$R/ram/carrier-scratch
export CARGO_BUILD_JOBS=12
mkdir -p $TMPDIR $SOLID_CHECKER_FUZZ_SCRATCH
out=$R/runs/carrier-$tag
mkdir -p $out
log() { echo "$(date '+%F %T') $*" | tee -a $out/status; }
# Memory guard: any process of this script's tree over 12 GB RSS is killed.
( me=$$; while kill -0 $me 2>/dev/null; do
    for p in $(pgrep -s $(ps -o sid= -p $me | tr -d ' ') 2>/dev/null); do
      rss=$(awk '/VmRSS/{print $2}' /proc/$p/status 2>/dev/null); [ -n "$rss" ] && [ "$rss" -gt 12582912 ] && { echo "guard: kill $p rss ${rss}kB" >> $out/status; kill -9 $p; }
    done; sleep 2; done ) &
for b in old new; do
  src=$H/wt; [ $b = old ] && src=$H/base-wt
  dst=$R/carrier-$b
  mkdir -p $dst
  rsync -a --delete --exclude /rust/target --exclude /bin --exclude /.git --exclude 'node_modules' --exclude /target $src/ $dst/
  for dir in scripts/fuzz/install scripts/runtime-oracle-install packages/cli; do
    [ -e $dst/$dir/node_modules ] || ln -s $R/solid-checker/$dir/node_modules $dst/$dir/node_modules
  done
  log "build $b"
  (cd $dst && make -s build-checker-release > $out/build-$b.log 2>&1) || { log "build $b failed"; exit 1; }
  sha256sum $dst/rust/target/release/solid-checker-rust | tee -a $out/status
done
gen=$R/carrier-new
for b in old new; do
  bin=$R/carrier-$b/rust/target/release/solid-checker-rust
  tf=$R/carrier-$b/bin/solid-typefacts
  cd $gen
  log "run $b mx SRU"
  node scripts/fuzz/matrix.mjs --grid --recall --seed 1 --codes STRICT_READ_UNTRACKED --contexts body,effect-apply,root --ids $H/ids/mx-recall-STRICT_READ_UNTRACKED.json --out $out/$b/mx-SRU --confirm-with-chrome --checker-bin $bin --typefacts $tf > $out/$b-mx-SRU.log 2>&1; log "rc=$?"
  log "run $b mx SWOS"
  node scripts/fuzz/matrix.mjs --grid --recall --seed 1 --codes STORE_WRITE_OUTSIDE_SETTER --contexts handler,timer --ids $H/ids/mx-recall-STORE_WRITE_OUTSIDE_SETTER.json --out $out/$b/mx-SWOS --confirm-with-chrome --checker-bin $bin --typefacts $tf > $out/$b-mx-SWOS.log 2>&1; log "rc=$?"
  log "run $b lg SRU"
  node scripts/fuzz/legal.mjs --grid --recall --seed 1 --codes STRICT_READ_UNTRACKED --contexts effect-compute,memo --ids $H/ids/lg-recall-STRICT_READ_UNTRACKED.json --out $out/$b/lg-SRU --confirm-with-chrome --checker-bin $bin --typefacts $tf > $out/$b-lg-SRU.log 2>&1; log "rc=$?"
done
log done
