# Memory index

- [Corpus location](corpus-location.md) — the rc.17 sweep corpus is on the owner's other machine, not rebuildable from the repo
- [Toolchain via mise](toolchain-via-mise.md) — node/bun/rust come from mise; activate it before make or cargo in non-interactive shells
- [Upstream parity policy](upstream-parity-policy.md) — superset scope, upstream names unless inaccurate, semantic rules only (ADR 0381)
- [Corpus sweep apps-root](corpus-sweep-apps-root.md) — always pass --apps-root ../solid-checker-rc17-corpus/apps and the rc17 manifest to corpus-sweep.mjs
- [Series handoff 2026-10-25](series-handoff-2026-10-25.md) — PR #116 pushed through ADR 0403 (11f04f5fe); final sweep + Opus review remain; moving machines
- [Shell noclobber and test runners](shell-noclobber-and-test-runners.md) — use >| in Bash; packages/cli tests are bun+vitest; verify-delta escalates to full verify
- [No broad pkill](no-broad-pkill.md) — kill only your own PIDs; other worktrees run the same scripts
- [Reproduction set direction](repro-set-direction.md) — replace the big corpus with a small budgeted in-repo set of uncertifiable reproductions
