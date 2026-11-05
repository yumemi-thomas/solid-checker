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
