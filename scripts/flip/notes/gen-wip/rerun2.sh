#!/bin/zsh
source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"
S=$HOME/EDF/tmp/flip-gen-await; cd $S/wt || exit 2
cargo +1.99.1 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test backend_process -- a_relinked_workspace_package_is_resolved_again 2>&1 | grep -E "^test |test result|left:|right:"
echo "relink rc=${pipestatus[1]}"
t0=$(date +%s); make verify > $S/verify2.log 2>&1; rc=$?; echo "verify2 rc=$rc $(( $(date +%s)-t0 ))s"; echo "$(date '+%F %T') verify2 rc=$rc $(( $(date +%s)-t0 ))s" >> $S/gate-times.tsv
