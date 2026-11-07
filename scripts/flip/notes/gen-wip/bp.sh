#!/bin/zsh
source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"
S=$HOME/EDF/tmp/flip-gen-await
for i in 1 2; do for side in wt base-wt; do
  cd $S/$side || exit 2
  out=$(cargo +1.99.1 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test backend_process 2>&1); rc=$?
  echo "run $i $side rc=$rc load=$(sysctl -n vm.loadavg) $(echo "$out" | grep -E '^test result')"
  echo "$out" | grep -E "^test .* FAILED|assertion .* failed:"
done; done
