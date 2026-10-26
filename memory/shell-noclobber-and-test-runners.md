---
name: shell-noclobber-and-test-runners
description: "Bash tool shell has noclobber on (use >| or the Write tool); packages/cli tests run through bun+vitest, not node --test"
metadata:
  node_type: memory
  type: reference
  originSessionId: 5c7d80e1-783f-4e43-841a-a680da8a94c1
  modified: 2026-10-25T22:31:47.730Z
---

In this repo's Bash tool shell, `>` onto an existing file fails with "file exists" and, inside a `&&` chain, silently skips every later command (a heredoc `cat > f` in a chain left a fixture half-written). Use `>|` or the Write tool for overwrites.

`packages/cli` tests are vitest run by bun: `bun ./node_modules/vitest/vitest.mjs run --config vitest.config.mjs test/adapter.test.mjs` (from `packages/cli`, with `$(mise which bun)` because `bun` is a safe-chain shell function). `node --test` on them hangs.

`make verify-delta` escalates to the full `make verify` for this branch (788 unmapped changed paths); the full verify took about 6 minutes on a quiet machine on 2026-10-26.
