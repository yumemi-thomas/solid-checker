---
name: repro-set-direction
description: Owner's 2026-10-26 strategy - replace the 13 GB corpus with a small in-repo set of reproductions of uncertifiable causes, under hard size budgets
metadata:
  type: project
---

Owner decision (2026-10-26): stop depending on the big rc.17 corpus for routine precision work. Build a light reproduction set in the repo (Opus 5.5 task task-repro-set-r1, branch `repro-uncertifiable` from PR #116 head 11f04f5fe): few small projects (one shared project per main rule for local causes, isolated projects only for project-wide premises such as the dispatch veto ADR 0356, open boundary ADR 0402, census ADR 0403; ~7, max 8), real published package bytes in one shared node_modules, tsc-clean, causes with >= ~20 corpus occurrences, one CASES table with provenance and weight, hard budgets enforced by `make verify`. Adding a new repo = sweep it once, group causes, add missing reproductions; the repo is not kept.

**Why:** the owner wants things light and portable across machines; the corpus is 13 GB and skewed (app-game 47%).

**How to apply:** iterate precision slices on the reproduction set; use a real corpus only to validate reproductions are faithful (see [[corpus-location]]); keep reproductions sound (no stubs looser than real typings). The corpus freeze (~/EDF/tmp/corpus-freeze) is optional archive, not the main path.

Process decision (owner, 2026-10-26): no continuous intake; user issues with minimal repros cover what users see. Before each release, run once locally against many pinned public Solid 2 repos (versioned release manifest, restored then discarded) and reproduce, one case per cause/shape: uncertifiable causes, triaged violations (false positive = checker fix + negative case, blocks the release), and sampled clean shapes confirmed by runtime probe or mutation (clean cases guard precision). Violation and clean cases get their own budget lines; cases table has a kind column.

Status 2026-10-26 evening: set committed and pushed on PR #116 (973d5f7b1, 8a2e977db; ADR 0404; 139 cases, 8 projects, 2,857 lines, 3.4 MB). Its "97.4% coverage" is label-prefix coverage; an independent Opus review estimated 35-55% mechanism-faithful, found no false-positive guard (no violation/clean cases), a loop-await classifier bug, veto mis-attribution. Owner rule: delete the corpus only after a second review says yes; fixes were sent to the same agent thread (archive sources, labelWeight vs verifiedWeight, split mixed buckets, violation triage + violation/clean cases).
