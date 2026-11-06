# Flip Track R, parallel agents: shared rules

solid-checker: a Rust engine that proves Solid 2 reactivity defects. Read `AGENTS.md`, `scripts/flip/PLAN.md`, and the plan reports named in your task. Start no subagents.

## Owner's rules
- What is not proven is a clear uncertifiable with a named blocker, never a silent clean, however rare. Clean only with proof, warn only with proof, for every rule. Prefer a proved violation where the fact is local. Counts are no reason to narrow.
- A violation means "this code is wrong if it runs". A branch the checker cannot evaluate counts as reachable.
- Every real false clean or false positive: fuzz shape first, old build fails, new passes, `scripts/fuzz/rediscovery.json` entry with ids (prefer that over `outside`). A report's "after" is inferred until you build it. Reproduce in real Chrome and `tsc` yourself, and report anything that does not reproduce.
- Never clear a row by changing the judge, a test assertion, a limit or a classification, unless you have shown it is a measurement artifact and removed it where it arises, with its own counter. Say so in your report before you do it. Earlier agents got this wrong.
- Chain commits to real exit status with `&&`, never `;`. No wrapper that hides an exit code.
- Never claim a count you did not measure on that exact build. Read every moved violation one by one. Sample large uncertifiable rises, and put per-project numbers in the commit message. Do not stop or wait for a review: there is none per rise. Ignore any report line about a Fable review.
- Say "proved" and "now uncertifiable" separately. Never say "fixed" for something that only became uncertifiable.
- Never `git reset --hard`, `git checkout --`, `git clean`. Never force-push. Never touch `claude/solid-rc21` (#173). No merge to main.
- Do not block on a prompt. Make the conservative call, record it, keep going.

## Speed
- Whole `mx-recall` and `lg-recall` run once, at the very end, by someone else. Do not start them. While fixing, use `make fuzz-focus`, `make fuzz-patterns`, and affected row ids.
- Group small fixes that do not interact into one check cycle. If a batch fails, split it. A full cycle is about 6 minutes.
- Log each gate to your scratch `gate-times.tsv`: date, commit, gate, seconds, exit.

## Machine
- One Cargo process per `rust/target`. Stay in your own worktree. Your scratch is the directory named in your task.
- `sc-heavy` is the one-at-a-time lock. Use it for `make verify`, sweeps, full coverage and release builds. Never nest it: a script that already takes the lock, wrapped again, deadlocks forever.
- The helpers in `/tmp/sc-scratch.noindex/flip-reads/` (`bld.sh`, `sw.sh`, `chk.sh`, `verify-seq.sh`) hardcode the Track R worktree and its output paths. Copy them to your scratch and change every path. Do not run them as they are.
- Grid processes read `scripts/fuzz/*.mjs` live.
- `make verify`'s memory test `rebuild_and_update_loops_do_not_accumulate` can fail in parallel lanes. Its limit must not change. See `/tmp/sc-scratch.noindex/flip-reads/verify-seq.sh`.

## Sharing the branch
Everyone delivers to `claude/flip-reads`. At each green commit: `git fetch origin`, merge `origin/claude/flip-reads` into your branch, rerun `make verify-delta` and the sweep on the merged tree, then `git push origin HEAD:claude/flip-reads`. If the push is rejected, merge again. Do this often, at least per family, so conflicts stay small.
`blockers.rs`, `rediscovery.json`, `open-defects.json` and `cases.json` are append-only: edit by text and union at merge. Never reformat them with a JSON dump.
ADR numbers: see your task. `docs/decisions.md` is near its doc budget; do not raise a budget.

## Reporting
After each family, one short message: what is proved, what is now uncertifiable, row counts. Keep a section in `scripts/flip/notes/track-r-progress.md`, under your agent's name. When your context goes over 500k, stop at the next green commit, write a handoff section and report. Final report: done, left, proved and uncertifiable counts, open false cleans and false positives, gates with results, what needs the owner.
