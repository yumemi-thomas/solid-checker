#!/bin/zsh
# vm-round.sh <label>: (under the lock) sync the VM clone to wt, keep its release binaries as bins/<label>, then census gen cells + legal twins
# with them, and census-codes for CODES with bins/<label> and bins/v0850.
source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"
S=$HOME/EDF/tmp/flip-gen-await; H=$S/vm; label=$1; R=/mnt/lima-colima/sc/runs/gen-await
cd $S/wt && FUZZ_VM_PROFILE=sc scripts/fuzz/vm.sh sync > $S/sync-$label.log 2>&1 || exit 3
colima ssh --profile sc -- bash -lc "cd /mnt/lima-colima/sc/solid-checker && grep -q 'if !seen.insert(span)' rust/crates/solid-facts/src/ast/types.rs && git merge-base --is-ancestor e40429f5e HEAD && mkdir -p $R/bins/$label && cp rust/target/release/solid-checker-rust bin/solid-typefacts $R/bins/$label/ && sha256sum $R/bins/$label/*" || exit 4
run() { local name=$1; shift; colima ssh --profile sc -- bash -lc "cd /mnt/lima-colima/sc/solid-checker && rm -f $R/$name.done && (setsid nohup bash -c 'bash $H/guarded.sh $name $* ; touch $R/$name.done' >/dev/null 2>&1 < /dev/null &)"; until colima ssh --profile sc -- bash -lc "test -f $R/$name.done" 2>/dev/null; do sleep 15; done; colima ssh --profile sc -- bash -lc "cat $R/$name.status; grep -E 'rc=' $R/$name.log"; }
run $label bash $H/census.sh $label $R/bins/$label
[ -n "$CODES" ] && run $label-c bash $H/census-codes.sh $label $R/bins/$label $CODES
[ -n "$CODES" ] && [ -n "$BASE" ] && run $BASE-c bash $H/census-codes.sh $BASE $R/bins/$BASE $CODES
echo round done
