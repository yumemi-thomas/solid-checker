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
