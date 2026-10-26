---
name: toolchain-via-mise
description: "On this Mac, node/bun/rust come from mise (installed 2026-10-25); non-interactive shells need mise activation before make or cargo work"
metadata:
  node_type: memory
  type: project
  originSessionId: f7acefda-1e86-42dc-a3ef-0469297f2477
  modified: 2026-10-25T03:30:56.180Z
---

Installed 2026-10-25 at the owner's request: Homebrew `mise`, with global `node@lts` (24.x), `rust@1.99.1` (rustup-backed, so `cargo +1.99.1` works, rustfmt + clippy + the `wasm32-wasip1` and `wasm32-wasip1-threads` targets; `make verify` needs both, CONTRIBUTING lists only the first), `bun@1.4.0`, plus Homebrew `cmake` (producer's mimalloc). `~/.zshrc` has `eval "$(/opt/homebrew/bin/mise activate zsh)"`.

Also needed once on a fresh machine: `cargo +1.99.1 fetch` for both workspaces, because `make verify`'s compiler-identity step runs `cargo metadata --offline`.

**Why:** the Bash tool's non-interactive shell does not run `.zshrc`, so `make` fails with `node: command not found` unless mise is activated first.

Relocated 2026-10-25 (owner: binaries must live under ~/EDF, which is excluded from endpoint scanning): `RUSTUP_HOME`, `CARGO_HOME`, `MISE_DATA_DIR`, `npm_config_cache`, `BUN_INSTALL_CACHE_DIR` now point under `/Users/thomas.guehenneux/EDF/toolchains/{rustup,cargo,mise,npm-cache,bun/install-cache}`, exported from `~/.zshenv` (which also puts `$CARGO_HOME/bin` and /opt/homebrew/bin on PATH). Never let a tool recreate `~/.rustup`, `~/.cargo` or `~/.local/share/mise`: if a shell lacks the env, mise reinstalls there. Aikido `safe-chain` 1.5.26 is installed globally (wraps npm/npx/bun/pnpm/yarn in interactive zsh/bash).

**How to apply:** `source ~/.zshenv; eval "$(/opt/homebrew/bin/mise env -s zsh)"` at the start of a Bash call (the old prefix without `source ~/.zshenv` will miss cargo), or `export PATH="/opt/homebrew/bin:$PATH"; eval "$(mise env -s zsh)"` (or `mise exec -- make ...`). The release checker is `rust/target/release/solid-checker-rust`; the producer is `bin/solid-typefacts`; set `SOLID_CHECKER_DAEMON=0 SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts` when invoking the binary directly.

Defender (2026-10-26): `mdatp exclusion list` excludes folder `/Users/*/EDF` only (no process exclusions); Defender still scans writes outside it (~150% CPU during builds). Keep all work under ~/EDF: our scratch dirs moved to ~/EDF/tmp with /tmp/sc-* symlinks; T3 worktrees (~/.t3/worktrees, 24G) to be relocated under ~/EDF. Since 2026-10-26 ~/.zshenv sets TMPDIR=~/EDF/tmp/system/ (plus MISE_CACHE_DIR, BUN_RUNTIME_TRANSPILER_CACHE_PATH under ~/EDF/toolchains/caches); `make verify`'s wasm-parity needs a macOS /private alias, so scripts/verify.sh now puts that scratch in /private/tmp on Darwin (commit on the PR branch). ~/.t3/worktrees is a symlink to ~/EDF/t3-worktrees.

Scope (owner, 2026-10-26): the ~/EDF layout, TMPDIR relocation and Defender notes apply to THIS machine only; other machines the owner moves to have no such constraint, so handoff scripts and prompts must not hardcode ~/EDF paths.
