# Track R progress

Branch `claude/flip-reads`. One line per rule; fresh agents continue from here.

Helpers (this machine, scratch): `/tmp/sc-scratch.noindex/flip-reads`: `ff.sh <name> <bindir> <codes> --grids ...` (focused run), `sw.sh <tag> <prev>` (sweep build `r<tag>`, compare), `redisc.py` (append a rediscovery entry), `base/` (build at the branch cut), `sweep-base.json` (16,036 U, 383 V).

| Rule | State | Corpus move | Notes |
|---|---|---|---|
| SC5007 | done, ADR 0650 | 0 | old grid 9,540 false cleans, new 0; also a possibly undefined accessor is no longer a violation |

Next: SC8003, then the plan's order.

Surprises:
- **The branch cut's Rust tests are red, 3 of 1,252, independent of this track**: `changing_a_receivers_type_in_another_file_moves_the_veto`, `eslint_plugin_solid_two_corpus_matches_native_rule_semantics`, `eager_read_summaries_require_exact_fresh_stack_global_bindings`. Confirmed on a clean worktree at `895fbf806`. So `make verify-delta` fails its rust lane on every commit. Gate used: `nt.sh` (nextest, no fail-fast) must show no failure beyond these 3, and verify-delta's other lanes pass. Pass `BASE=895fbf806`, or it maps thousands of paths against `origin/main`.
- `scripts/fuzz/install` needs `npm ci` too (Chrome host); the plan names only the oracle install.
- `view` children in a generator context must be numbers: build a boundary in `body` with `domExpr`.
