#!/bin/zsh
source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"
for side in wt base-wt; do
  cd /Users/thomas.guehenneux/EDF/tmp/flip-gen-await/$side || exit 2
  echo "== $side $(git log -1 --format=%h)"
  cargo +1.99.1 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test backend_process -- daemon_edits_a_tracked_source_without_a_rebuild a_history_edited_in_another_file_moves_the_premise a_router_release_moved_between_audited_and_unaudited_is_resolved_again 2>&1 | grep -E "^test |test result|panicked|left:|right:"
  echo "cargo rc=${pipestatus[1]}"
done
cd /Users/thomas.guehenneux/EDF/tmp/flip-gen-await/wt && node /Users/thomas.guehenneux/EDF/Yumemi/GitHub/solid-checker/packages/cli/node_modules/vitest/vitest.mjs run --root /Users/thomas.guehenneux/EDF/tmp/flip-gen-await/wt --config /Users/thomas.guehenneux/EDF/Yumemi/GitHub/solid-checker/packages/cli/vitest.config.mjs scripts/fuzz/exec.test.mjs 2>&1 | tail -6
echo "exec rc=${pipestatus[1]}"
