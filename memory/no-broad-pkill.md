---
name: no-broad-pkill
description: never pkill -f by script name (scripts/verify.sh): other worktrees' agents run the same scripts
metadata:
  type: feedback
---

Do not stop a run with `pkill -f scripts/verify.sh` or similar name patterns: parallel agents in other worktrees run the same scripts and the pattern kills their `make verify` too (it happened once, exit 130 in another agent's log).

**Why:** several agents share the machine; process names carry no worktree.
**How to apply:** stop only a PID you started (`$!`) or match the worktree path in the pattern; otherwise let the run finish. See [[shell-noclobber-and-test-runners]].
