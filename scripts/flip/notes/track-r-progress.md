# Track R progress

Branch `claude/flip-reads`. One line per rule; fresh agents continue from here.

Helpers (this machine, scratch): `/tmp/sc-scratch.noindex/flip-reads`: `ff.sh <name> <bindir> <codes> --grids ...` (focused run), `sw.sh <tag> <prev>` (sweep build `r<tag>`, compare), `redisc.py` (append a rediscovery entry), `base/` (build at the branch cut), `sweep-base.json` (16,036 U, 383 V).

| Rule | State | Corpus move | Notes |
|---|---|---|---|
| SC5007 | done, ADR 0650 | 0 | old grid 9,540 false cleans, new 0; also a possibly undefined accessor is no longer a violation |

Next: SC8003, then the plan's order.

Surprises:
- The branch cut had 3 red Rust tests (none ran `make verify` there); fixed in the commit `test(flip-base): ...`, for Track W to pick up. Rust tests are 1,252 of 1,252 from then on; `nt.sh` prints anything new. Pass `BASE=<last rule commit>` to verify-delta, or it maps thousands of paths against `origin/main`.
- `scripts/fuzz/install` needs `npm ci` too (Chrome host); the plan names only the oracle install.
- `view` children in a generator context must be numbers: build a boundary in `body` with `domExpr`.
