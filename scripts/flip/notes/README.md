# Notes copied from the first machine

Verbatim, for the second machine. They describe the state before `claude/flip-base`. Where they disagree with `../PLAN.md`, the plan wins.

| File | What it is |
|---|---|
| `handoff.md` | State of `claude/rc21-open-items` at `ea16edb79`, and working knowledge: surprises, gates, what CI failed on |
| `queue.md` | The #173 work queue. Sections 1-3 name what the flip covers; item numbers are its Q1-Q6 |
| `v-d1-checker.patch` | Ready, unapplied fix for SC2003 no-op writes (queue item 1). Use it in W's SC2003 commit. Written against `ea16edb79` |
| `cost-split.md` | Uncertifiable sites per project by mechanism, up to `6730781e7`. Last line: kinds before and after |
| `missing.md` | Surface agent: what the fuzzer does not generate. Input to the DOM rows in the plan |
| `redundancy.md` | Surface agent: which fuzz values are redundant, and the `<Portal>` gap |

## Design notes, which beat `../PLAN.md` where they differ

Read at `ef46d0ac6`. Their programs are in `/tmp/sc-scratch.noindex/flip-reads/research/probes/` on the read track's machine only.

| File | What it is |
|---|---|
| `sc1001.md` | SC1001's steps 0-9, the `unplaced_run` hole at the call, setter callbacks, the 105 corpus sites |
| `f0.md` | `function_summary_complete`: the predicate, order, D4 first |
| `sc9012-census.md` | The census positions, H-D5, what owns what, the 8 W witnesses |
| `async.md` | SC5003 + SC5001 (three facts, not one), SC5006, SC5008: a violation needs the source pending and the first run inside `render()`. Read at `076ad9a8b` |

Corrections to the plan, from the owner:

- "`reconcile` takes 1 argument" is wrong: `setStore` does, so that part of C-D2 is live.
- SC1001's `:2577` edit does nothing. Skip it.
- Q4, Q5, C-D4 and H-D5 had no owner. `j08` was claimed by both tracks. These, and any item with no owner, are Track R's unless it is a write.
- The file table lacks these: `reactive_analysis.rs` (both tracks edit it, in separate hunks: W after `transported_writes`, R at `:219` and `:327`), `result_reads.rs`, `props_reactivity.rs`, `value_use.rs` (new) are R's. `blockers.rs` is append-only for both.

From `async.md`, for Track W: step 6 feeds `async_sources`, which SC2009 reads, so an `any`-computed `createOptimistic` starts to hold. `server_rules.rs:191` `loading_wrapped_components` stays W's, separate from `loading_cover_at`. `AsyncRead` gains fields near W's hunks.

For Track W: `u11` is yours and needs the fix at the call, not only `unreached_write` (`sc1001.md` section 7). Do not put setter callbacks in `layer_run`. Does SC2003's forward flow reach `w(s.o)`? If not, `j05` has no owner (`sc9012-census.md` section 8).

## Paths that exist only on the first machine

Nothing below can be opened here. Treat each as a label, not a location.

| Written as | What it was | Use here |
|---|---|---|
| `$O`, `/tmp/sc-scratch.noindex/rc21-open` | Scratch: binaries, sweeps, helper scripts (`probe.mjs`, `chk.mjs`, `snapdiff.py`, `cost3.py`, `gx.sh`) | Not copied, except this patch. `scripts/fuzz/open-defects.json`'s `$O/v-d1-checker.patch` is `scripts/flip/notes/v-d1-checker.patch` |
| `costbin*`, `sweep-cost*.json`, `tgt-base/`, `cur/` | Checker builds and sweep reports | Build your own baseline from the branch cut |
| `/tmp/sc-scratch.noindex/small-proofs/...`, `premise-audit/...`, `fuzz-surface/j3/` | Audit reports and probes | Not copied. Their programs are in `scripts/fuzz/patterns/` and `../witnesses.json` |
| `/Users/thomas/dev/solid-checker/t3-4ac868af`, `/Users/thomas/dev/sc-worktrees.noindex/...` | Worktrees | Your own worktree |
| `/Users/thomas/Documents/Github/solid-checker-rc21-frozen/apps` | The frozen corpus | Needed by `--apps-root`. Check you have it, at the manifest's pins |
| `/Users/thomas/.local/bin/sc-heavy` | One-slot lock for heavy steps | Whatever serialises heavy runs on your machine. The rule stands: one at a time |

## Instructions in these notes that no longer apply

- `queue.md`'s holds on section 2, and its "fewer reviews" and "patterns only" changes: the flip replaces them. Build by `../PLAN.md`.
- `queue.md`'s estimates and the "reject" on cost: counts are not a reason to narrow a rule. `PLAN.md`'s SC2003 row says which design stays.
- `handoff.md`'s "branch `claude/rc21-open-items`, tree clean": work on the branch this directory is on.
- `handoff.md`'s "do not push" stands: push only on the owner's word.
