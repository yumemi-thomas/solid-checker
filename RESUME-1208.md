# RESUME ADR 1208 (W10, paused 2026-11-25 by the owner)

Branch `claude/store-prop-source-cells-1208` (worktree `~/dev/sc-worktrees.noindex/w10-1208`, base origin/main 875c8a07c).

Done:
- `scripts/fuzz/matrix.mjs` `recallShapesOf`: store variants of STRICT_READ_UNTRACKED and STORE_WRITE_OUTSIDE_SETTER on
  their base cells crossed with prop, prop-destructured, context, context-destructured (direct + const alias, no layer),
  misuse and legal: 6,168 mx-recall + 4,304 lg-recall new programs.
- Run on main 875c8a07c in the sc VM (fast path on all; real Chrome `--runtime-diff all` on every false result, 168/168
  same; stratified Chrome sample 512/512 same). Runs: `~/EDF/tmp.noindex/w10/runs/main08{,f,s,t}`, ids `sel08*`.
- ADR 1208 entry in docs/decisions.md lists the findings.

False results found (not fixed):
1. 6 false positives: lg-recall STORE_WRITE_OUTSIDE_SETTER noop-and / noop-nullish through `prop` (handler, then, timer):
   `props.s1.v &&= 5` over `createStore({ v: 0 })` reports SC2003; nothing is written (Chrome silent); local twin clean.
2. 16 certified false cleans: STRICT_READ_UNTRACKED store-dstr-merge through context / context-destructured (body,
   effect-apply, render-apply, root): `const { v } = merge(ctx.s1, { w: 1 })` has no finding; Chrome warns; local twin SC1003 V.
3. 146 false cleans with only an unrelated SC9012 near the site: store-dstr-for-of/-assign/-merge (prop sources),
   store-write net-sort-call/-reflect/-bind, net-push-reflect/-apply-held (all four sources), sw-await/-generator/-let/
   -param-plain/-rest/-then/-assign-copy (prop-destructured). Local twins recall-gap or agree.

Next:
- Wait for the coordinator's call on fixing the three classes.
- Run verify-delta in the VM clone `w10/c`: info/exclude the symlinks, COUNT_BASE, CARGO_INCREMENTAL=0,
  `make tsc-oracle-provision`. The local fuzz unit tests fail here only because the worktree has no node_modules.
- Note: `--runtime-diff 0` judges on the fast path only, so pass `--runtime-diff all` (or N) for Chrome.

## Rerun of the context store cells against ADR 1211 (2026-11-26, W10)

1208's generator (c244c183d) on its 5,236 context cells (sources `context`, `context-destructured`: mx-recall 3,084, lg-recall 2,152),
main-release 7bc723e35 (1211's base) against a release build of 1211 (5e84277da), real Chrome on every program (`--runtime-diff all`:
2,988 + 2,148 compared per run, 0 behaviour, 0 attribution, 0 inconclusive). Tools and rows: `~/EDF/tmp.noindex/w10/r1208`.
- 1211 moves 372 mx-recall programs recall-gap -> agree (store writes through a context), nothing else: no lg-recall move, no new false
  positive, no new false clean.
- Unchanged on both builds, 68 mx-recall false cleans in the context cells:
  - class 2 (16, certified): STRICT_READ_UNTRACKED `store-dstr-merge` through `context` / `context-destructured` (8 each).
  - class 3 (52, an unrelated SC9012 near the site): `store-dstr-for-of` 16 and `store-dstr-assign` 8 (strict reads), and
    STORE_WRITE_OUTSIDE_SETTER `net-sort-call`, `net-sort-reflect`, `net-sort-bind`, `net-push-reflect`, `net-push-apply-held` 28.
- Class 1 (the 6 noop-and/noop-nullish false positives) is a `prop` cell, not in this rerun.
