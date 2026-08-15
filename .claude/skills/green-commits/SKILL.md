---
name: green-commits
description: Slice a large dirty solid-checker worktree into individually green commits — backup branch, checkpoint-file staging, per-slice gates, and the end-of-series fmt/clippy sweep. Use when splitting accumulated work into commits, preparing a PR from a big worktree, or whenever every commit must pass the gates on its own.
---

# Slicing a big worktree into green commits

The house bar: **every commit individually green on all gates** (see
`.claude/skills/verify-handoff/SKILL.md` for the gates). Verify by checking
out each commit and running the oracle — a small bisect script beats trusting
the grouping by eye. This procedure was proven on PR #4.

## Before anything

Commit the full working state to a backup branch. Every later step is checked
against it; nothing is recoverable without it.

## Per slice

1. **Stage the slice.** For a file whose final content spans multiple slices,
   use the checkpoint-file technique: copy the final version to scratch,
   `git checkout HEAD -- <file>`, edit it to the intermediate state this
   slice owns, `git add` it, then copy the final version back into the
   worktree.
2. Hide everything unstaged: `git stash push -u --keep-index`.
3. Run the gates against the staged state. Regenerate derived artifacts
   against that state too (rule manifests via their update env, snapshots via
   `coverage.mjs --update`) so the commit is self-consistent — a snapshot
   regenerated against the final state does not match an intermediate commit.
4. Commit.
5. `git stash pop`, resolving any conflicts toward the stash (the stash holds
   the newer, full-worktree content).
6. **Drift check after every slice:** with everything added, `git diff
   <backup-branch>` must be empty. Any output means a pop-conflict resolution
   lost content — fix it now, not three slices later.

## End of series

Sweep fmt and clippy **per commit** in a spare worktree — synthesized
intermediate states fail rustfmt easily even when the final tree is clean.
Fix an offender by amending that commit and replaying the rest with
`git rebase --onto <amended> <old> -X theirs`, then assert the series tip's
tree hash is unchanged (`git rev-parse <tip>^{tree}` before and after).

## Slice-boundary traps

- **The dialect seam moves as one piece.** When the seam changes
  (vocabulary methods, defect-vs-violation projection), solid-dialect,
  the solid-reactive-ir engine, and both rules catalogs cannot land in
  separate commits — no intermediate compiles.
- **A new fixture's node_modules stub needs its `.gitignore` exception lines
  in the same commit**, or the fixture un-dialects only in CI (see
  `.claude/skills/add-fixture/SKILL.md`).
- **Snapshot updates belong in the commit whose code moved the findings**,
  not the thematically nearest one.
