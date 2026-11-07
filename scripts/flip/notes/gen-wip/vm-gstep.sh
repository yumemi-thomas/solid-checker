#!/bin/zsh
# (one lock hold) sync the VM clone to wt (the fuzz scripts with gstep-*; release binaries rebuilt as bins/<label>), then census-ctx gstep-* with <label> and with each extra label.
source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"
S=$HOME/EDF/tmp/flip-gen-await; H=$S/vm; R=/mnt/lima-colima/sc/runs/gen-await; label=$1; shift
cd $S/wt && FUZZ_VM_PROFILE=sc scripts/fuzz/vm.sh sync > $S/sync-$label.log 2>&1 || exit 3
colima ssh --profile sc -- bash -lc "cd /mnt/lima-colima/sc/solid-checker && grep -q 'if !seen.insert(span)' rust/crates/solid-facts/src/ast/types.rs && git merge-base --is-ancestor e40429f5e HEAD && mkdir -p $R/bins/$label && cp rust/target/release/solid-checker-rust bin/solid-typefacts $R/bins/$label/ && sha256sum $R/bins/$label/*" || exit 4
C=gstep-handler,gstep-timer,gstep-kept,gstep-returned,gstep-spread,gstep-from,gstep-destructure,gstep-delegate,gstep-method,gstep-unstepped-handler
for l in $label "$@"; do
  name=$l-gstep
  colima ssh --profile sc -- bash -lc "cd /mnt/lima-colima/sc/solid-checker && rm -f $R/$name.done && (setsid nohup bash -c 'bash $H/guarded.sh $name bash $H/census-ctx.sh $l-g $R/bins/$l $C; touch $R/$name.done' >/dev/null 2>&1 < /dev/null &)"
  until colima ssh --profile sc -- bash -lc "test -f $R/$name.done" 2>/dev/null; do sleep 15; done
  colima ssh --profile sc -- bash -lc "cat $R/$name.status; grep -E 'rc=' $R/$name.log"
done
echo gstep done
