# Handoff: PR #173, solid-rc21 false cleans

**State:** finished. Branch `claude/rc21-open-items`, head `ea16edb79a75a3799af8a33250874aed3b3dc44e`, 8 local commits past `origin/claude/solid-rc21` (`207b13142`). Tree clean. **Do not push: the owner pushes.** A later agent builds the flip on this head.

**What this head is:** ADRs 0619-0646 proofs and obligations, plus every audit program in `scripts/fuzz/patterns/` and the open list `scripts/fuzz/open-defects.json`. Do not branch from `a61d4bdc7`: it overwrote `open-defects.json` and fails `falseclean.test.mjs`.

**Open defects:** 393 programs of 1,137 and 540 lg-recall rows (V-D1), by `make fuzz-patterns` and any grid run. This is the flip's input.

| item | false positive | false clean | no run | fine |
|---|---|---|---|---|
| C-D1/T-D3 | 9 |  |  | 18 |
| C-D2 | 41 |  |  | 76 |
| C-D3 value-use |  | 37 |  | 34 |
| C-D4 | 3 |  |  | 5 |
| C-async-generator |  |  |  | 1 |
| C-inner-call-identity |  | 1 |  |  |
| H-D1 (F0) |  | 23 |  | 150 |
| H-D2 |  | 6 |  |  |
| H-D4 | 3 | 2 |  | 7 |
| H-D5 |  | 5 |  | 2 |
| H-capture |  | 4 |  | 1 |
| H-other | 3 |  |  |  |
| T-D1 | 2 | 12 |  | 18 |
| T-D2 | 2 | 12 |  | 12 |
| T-D3 | 11 | 1 |  | 12 |
| T-D4 |  | 2 |  |  |
| T-mount-from-cleanup | 1 |  |  |  |
| T-other | 4 | 40 | 2 | 219 |
| T-subclass-sort |  |  |  | 1 |
| T-subclass-sort-control |  |  |  | 1 |
| T-untrack-label |  | 1 |  |  |
| V-D1 | 16 | 1 |  | 22 |
| V-D2 |  | 22 |  | 30 |
| V-D3 |  | 14 |  | 7 |
| V-D4 |  | 9 |  | 16 |
| V-D5 |  | 18 |  | 7 |
| V-D6 |  | 4 |  | 1 |
| V-D7 |  | 4 |  | 2 |
| V-D8 |  | 6 |  | 1 |
| V-D9 |  | 2 |  | 2 |
| section-4 census:a |  | 18 |  | 14 |
| section-4 census:b | 2 | 1 |  | 5 |
| section-4 census:c | 2 |  |  | 6 |
| section-4 census:d |  |  |  | 1 |
| section-4 census:e |  | 20 |  | 12 |
| section-4 census:f | 1 | 3 | 1 | 6 |
| section-4 census:g | 3 |  |  | 3 |
| section-4 census:h | 1 | 3 |  | 22 |
| section-4 census:j | 2 | 2 |  | 15 |
| section-4 census:k | 2 | 5 |  | 10 |
| section-4 census:s | 1 |  |  | 4 |
| section-4 census:w |  | 3 |  | 1 |
| **total** | **109** | **281** | **3** | **744** |

`certified` in the list marks 267 false cleans in programs with no uncertifiable finding. "False positive" in the lane means a violation Chrome did not report; there are no hit counters, so some may be unexecuted sites: read each before fixing. `notGenerated` in the list says what is not a generator shape yet. A fix turns a program green, then `make fuzz-patterns` fails with FIXED until the entry moves to `rediscovery.json` with a generator shape.

**Ready fix, not applied:** `$O/v-d1-checker.patch`, V-D1 measured: 540 false positives to 0, corpus 0, `logical-or` loses its proof on rows where the setter is referenced. `/tmp` is not on another machine: take the patch with you.

**Not run:** `make verify`, grids, recall, final order, CI on these commits.

---

## Where

- Repo worktree: `/Users/thomas/dev/solid-checker/t3-4ac868af`, local branch `claude/rc21-open-items`.
- Delivery: PR https://github.com/yumemi-thomas/solid-checker/pull/173, branch `claude/solid-rc21`.
- **DO NOT PUSH.** Owner's instruction, in force until the owner says otherwise: commit locally on `claude/rc21-open-items`, one family at a time, and no `git push`. It replaces every earlier "push what you commit". Last pushed: `207b13142`.
  No `git push` of any kind, including in scripts. Do not undo `207b13142`: no force-push, no reset; it stays where it is. Never merge.
- Scratch: `O=/tmp/sc-scratch.noindex/rc21-open`.
- Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. PR body ends `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.

## Owner's rules (standing)

- False positives and false cleans are an absolute no. Every real false clean found is solved in #173; nothing is deferred. #173 stays open until the owner says all reports are in.
- More reports will come: premise audits D1-D9, T1-T4, the API census, after their Opus reviews.
- Say "proved" or "now uncertifiable" per item, never "fixed". Uncertifiable only where no compile-time fact decides it, or as an interim step with the proof tracked and a named blocker.
- One item at a time, fuzzer first: add the pattern to the generators, show the old build fails, fix, show the new build passes, add a `scripts/fuzz/rediscovery.json` entry with `ids` (never `outside` unless the generator truly cannot express it, and say why), commit. Small commits, one family each, each with its old and new evidence in the message.
- **Do not start the final order or any full grid run until the owner says all reports are in.** Focused runs per family only. No `make verify` yet.
- Reproduce every report item in real Chrome before fixing. Report anything that does not reproduce; never drop it silently.
- Snapshots: read every moved finding before `coverage-update`, in the same commit as the code. `snapdiff.py` prints them after an update.
- Tell the owner the state when each commit lands.
- Heavy steps run under `/Users/thomas/.local/bin/sc-heavy`, one heavy run at a time, no taskpolicy or nice. One Cargo per target. Kill only your own PIDs (`python3 $O/killtree.py <pid>`). Never `git reset --hard`, `git checkout --`, `git clean`.
- Replies short and plain.

## Committed on #173 (latest first)

- `3959e664e` (LOCAL) the last ruling shapes, `b02`, `b09`.
- `6730781e7` (LOCAL) item 8: `Unknown` sites on `any`/interface receivers. Old 85 false cleans, new 0. Corpus +4.
- `91ecc5757` (LOCAL) the `remove` exclusion removed. Corpus 0. No build fails on it: no rediscovery row.
- `ea0a63fdb` (LOCAL) item 6: `execCommand`, `ElementInternals`, `HTMLSelectElement.remove`. Old 34, new 0. Corpus 0.
- `503efaa84` (LOCAL) ruling items 2-5: denied events. Old 289 false cleans in 23 `dny-*` contexts, new 0. Corpus 15,891 to 16,032. Includes C phases (`flush()`, disposer, `insert`) and the user axis in-program.
- `207b13142` ruling item 7: an apply's delivery respects the flush veto (`g05`, `g10`). Old 3 certified false cleans, new 0; corpus cost 0. Fixtures `rc21-apply-owned-flush`, `rc21-apply-unowned-flush`.
- `b05b96574` ADRs 0643-0646: install-site obligation, render in the flush census, anchor census and `createElement("a")`, listener reach, S0, named zero-run carrier; rediscovery for everything sent so far; 6 of 10 `outside` converted.
- `5b8a1d7b5` judge: a contradicted violation is a false positive, never wrong-rule; audit sample key; gate test.
- `2394b10cd` ADR 0642, sync dispatch hazards.
- `7e9b9d5ee` and earlier: ADRs 0619-0641.

`outside` still (4): string-code census (no eval in the generator), SC7005 (SSR host is its own run), flush in a parameter default (could be a shape; not done), dynamic hang (no runtime record).

## In progress: the Opus sync-dispatch ruling

Read `/tmp/sc-scratch.noindex/small-proofs/veto/sync-adversarial.md` in full; probes in `sync-adversarial/`. Section 4 is the plan and is **withhold-only**. The owner's list, in order, each with its own commit:

1. Keep ADR 0642's list.
2. DONE, `503efaa84`.
3. DONE, `503efaa84`.
4. DONE, `503efaa84`.
5. DONE, `503efaa84`.
6. DONE, `ea0a63fdb`.
7. DONE, `207b13142`.
8. DONE, `6730781e7`: costs +4, far under the stop rule.

Also:
- Remove the `remove` exclusion (`ChildNode.remove` to `NoListener` in `event_dispatch_sites`), do not patch it; its premise is false. `click`'s `NoListener` stays.
- Check closed by the denied family: `flush()` in a body, `createRoot(() => flush())`, `render()`'s disposer in a body, memo, effect compute, `untrack`, `insert()` in a body (`c01`-`c05`, `c14`, `c20`).
- The "New false cleans" rows (1.3-1.7) closed or uncertifiable, with shapes, ids, old fails, new passes. Required by the ruling: this build must fail SC1001/SC2001 on `a01`, `b02`, `b05`, `c02`, `d09`, `e01`, `g10`, and SC2008 on `a04`, `b03`, `b09`.
- User axis in the fuzzer: typed text then focus loss, IME open, popover opened by click. The ruling's `prep.json` drives real input; say so in the entry if the harness cannot produce one.
- Update ADR 0642 (the `remove` premise, `form.submit`), `docs/open-work.md`, rule pages.
- `focus` and friends stay hazards: dropping them (about 1,392 back) is the stacked PR, not here. Do not build `DispatchReceiver::OffJsxPath`, S1, S2 here: stacked PR.
- Send the owner the per-project cost split by item before gates.

Ruling: built, cost sent (`$O/cost-split.md`). To finish sync dispatch: measure `c20`, `e02`, `e03`, `e04` with shapes, and the rule pages. Then the queue below, starting with C-D1, by the flip-the-default decision. Do not start gates.

## Scope now (owner, overrides the rest of this file)

**Add patterns only, then finish.** Fuzzer patterns for queue.md sections 1-3 and the section 4 axes, each run focused on the current build and recorded. NO checker code, in Rust or the rules. No fixes, no census, no flip. No rule page or ADR edits for them. A later agent builds the flip on this head. `$O/v-d1-checker.patch` is the finished, measured V-D1 fix, uncommitted and removed from the tree: 540 false positives to 0, `logical-or` loses its proof (54 agree to recall-gap, needs ADR 0491's literal on rows where the setter is referenced), corpus 0. Apply it with `git apply` when fixes are allowed.

## Work order now

`/tmp/sc-scratch.noindex/rc21-open/queue.md` is the order: section 1 (false positives) from the top, then the rest. **No review per increase:** do not stop or wait on a rise over 20; measure it, put per-project numbers in the commit message, read every removed finding yourself, keep going. **Hold section 2's census** until the owner relays the ruling on the shared flip mechanism (`premise-audit/flip/report.md`). Build sections 1 and 2 so that mechanism can absorb them. Still no push, no full grids.

## Owner's decision: value-flow families flip the default

Do not patch routes one at a time. Where a value reaches code the checker cannot follow, flip the default so the result is uncertifiable instead of clean, family by family, starting with the routes the values audit broke. For each family: sweep the corpus, report the cost by project, then re-prove the common cases. Uncertifiable is internal (ADR 0328), so this adds no user-visible noise; invariant 2 already requires it: a silent clean for an unfollowed value is the defect. It applies to the D1-D9 items and C-D1 to C-D5 when they reach you, and to the store-held-in-a-container gap under Open below. **Finish sync dispatch first.**

## Queue after the sync-dispatch ruling

False cleans, in #173, one family at a time, fuzzer first. Source `/tmp/sc-scratch.noindex/small-proofs/c1/adversarial.md`, probes `c1/adversarial/`; Chrome and `tsc` confirmed there, reproduce yourself. Names keep the `C-` prefix: other reports also use D1.

- **C-D1 (F0):** `function useT(n: () => number) { return { read: (): number => n() }; } const x = useT(n); x.read();` warns, certified. One unrelated `other.read = …` write gives SC9012, so the veto alone hides it. Slot bodies calling a parameter, a module `let` or a field. Cause: the net drop at `reactive_analysis.rs:219-236`. Fix: a resolved call drops the net only if the target's summary is complete. S0 closed one arm: check whether it needs widening. Upper bound about 400 added SC9012, name-based: measure. **Above 20 it needs review: send the owner the numbers.**
- **C-D4:** `__defineGetter__` and `valueOf()` on a closed receiver: any member call counts as a receiver. A false positive and a false clean.
- **C-D5:** `<C …><Dynamic component={C} run={setN}/></C>` plus 3 variants; Chrome throws. Cause at `props_reactivity.rs:723-733`.
- **C-D2:** `[{ run: n }]` iterated. **C-D3:** `get().read()`. Not traced.

Do not build S1, S2, the bypass, the C3 slice, `OffJsxPath` or the literal allowlist: proofs, they wait. If that review's order mentions an event-scoped veto, ignore it: it did not see `sync-adversarial.md`, which ruled it unsound.

## Baselines and tools (all under `$O`)

Binaries, each a fresh path: never `cp` over a binary that has run (macOS SIGKILLs it, `checker: exit null`).
`cur/` 80c55318c; `old15/` before ADR 0641's onSettled and tag fix; `tgt-base/release/` 7e9b9d5ee; `costbin/` 2394b10cd; `costbin4/` before S0; `costbin5/` S0; `costbin6`; `costbin7/` = b05b96574 logic, `costbin8/` = 207b13142, `costbin14/` = 503efaa84, `costbin19/` = 6730781e7 (`bin/solid-typefacts` beside each). Next family: `mkdir costbinN; cp rust/target/release/solid-checker-rust bin/solid-typefacts costbinN/`.

Scripts:
- `probe.mjs <dir> [filter]`: Chrome plus checker on cases `<id>/App.tsx`, `oracle.json`, `sites.json`. Env `BIN`, `TF`, `NOCHROME=1`, `NOCHECK=1`, `CLOSED=1`. Generators: `mkproto.py`, `mkrender.py`, `mkarb.py`, `mks0.py` show the case format.
- `chk.mjs <dir>`, `ctx.py <fixture> lo hi`, `rolelog.mjs <dir>` (role decisions, set to costbin6), `snapdiff.py` (moved findings against HEAD), `cost3.py a.json b.json` (sweep diff by mechanism), `redisc2.py` (how entries are built from rows; already applied, do not rerun it as is).
- Focused run: `node scripts/fuzz/focus.mjs --codes A,B --grids mx-recall,lg-recall --prune-cases --checker-bin D/solid-checker-rust --typefacts D/solid-typefacts --out $O/out/NAME -- --contexts a,b`. Rows: `out/NAME/<grid>/programs.jsonl`. Reused `--out` returns cached: `rm -rf` first. Exit 1 is normal when an old build fails.
- Corpus sweep, 3 to 4 minutes: `node scripts/corpus-sweep.mjs --manifest scripts/corpus/rc21-manifest.json --apps-root /Users/thomas/Documents/Github/solid-checker-rc21-frozen/apps --checker D/solid-checker-rust --typefacts D/solid-typefacts --order-from $O/sweep-cost7.json --out $O/sweep-costN.json`.
- `gx.sh <name> <bindir> <codes> <grids> <contexts>`: the focused run plus an outcome count by context.
- Waits: run a Bash loop on a done-file with `run_in_background: true`, then `mcp__t3-code__t3_background_wait` (load by ToolSearch).

Sweeps: `sweep-cost-base.json` (7e9b9d5ee) 13,554 U; `sweep-cost-new.json` (ADR 0642) 15,300; `sweep-cost3.json`; `sweep-cost5.json`; `sweep-cost8.json` (207b13142) 15,891 U; `sweep-cost14.json` (503efaa84) 16,032 U; `sweep-cost19.json` (6730781e7) **16,036 U, 383 V**. `costsplit.py` builds the per-project table. Cost so far by item: denied events +141; ADR 0642 +1,746; install 23; listener 488; S0 79; any receivers 4; render, census, created elements, zero-run 0. Violations never moved.

## Before each commit

`cargo +1.99.1 fmt --manifest-path rust/Cargo.toml --all`; clippy with `CARGO_TARGET_DIR=$O/tgt-test cargo +1.99.1 clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings`; `cargo test -p solid-dialect -p solid-reactive-ir --lib`; `make fuzz-test` (776 tests, about 150 s); `make repro-causes`; `make coverage` then read, then `make coverage-update`; `make tsc-oracle` when a rule's cases change; `node scripts/doc-budget.mjs` (raise budgets deliberately, name them in the commit).

CI failed before on: stale pins, clippy, `tsc-oracle` cases, and fuzz script tests. Run these locally, since CI will not be watched per commit.

## Surprises

- `ASYNC_OUTSIDE_LOADING_BOUNDARY` is in `SPEC` but not `MISUSE_CODES`, so it has no cells: its legal shapes live on `PENDING_ASYNC_UNTRACKED_READ`.
- `recallLegal` on a code goes to every code sharing `EVENT_LEGAL`: `rnd-*` legal is only legal for `REACTIVE_WRITE_IN_OWNED_SCOPE`, since an apply is a strict window.
- A `lst-` shape's local object behaves unlike a module-scope one; `lst-*-module` needs the `hoisted` statements. A fuzz row with a store Get found what signal-only probes missed: run store variants, not only `signal`.
- Adding to a table by first match can hit the wrong table in `misuse.mjs` (SPEC, then cbp tables).
- `sc-heavy` serialises heavy steps, so queue with a done-file chain.
- The zero-run fix sits at the end of `discover` on every read row, since a read row has several construction sites.
- `unexecuted` in a legal grid is a proved violation on a site that never ran: the gate excuses it, so read those rows yourself. That is how ADR 0646 was found.

- A denial must never make code roleless: a refused proof falls through to "no role" and goes silent. After any withhold-only change, diff the sweep's REMOVED findings (`cost3.py` prints them) and read each one. That found two regressions in the denied family. `denied_counts_marker` in `events.rs` is why the walk runs twice.
- rustfmt reflows code an edit script matches: a script that asserts stops before writing, so check its output; a stale `costbinN` then measures old code.
- `make fuzz-test` does not cover `dny-*` firing; the focused run does.

## Open, known and unfixed (owner informed of the first; fix under the new "nothing deferred" rule)

- **A store held anywhere but its own binding is not followed**: `const o = { s: store }; o.s.v`, `[store][0].v`, `Map.get`, `push`, `Object.assign(o, { s: store })`, `o.s = store`, `{ value: store }`, `read(store)` through a local helper, `o.s.v = 2`: Chrome warns, certified. In `docs/open-work.md`. A blanket obligation meets every `{ store, setStore }` a hook returns: needs its corpus number and the owner's call. Under "every real false clean is solved in #173" this is now in scope: raise it.
- Audit item 13 still silent, in open-work; items 14, 15, 17 in open-work.
- Judge: remote-claim link and excused count not done. SYNC_NODE_RECEIVED_ASYNC ref 5 rows: exempt via `partlySilentRules`, no run confirms; owner's call.
- `hydrate` in an apply or leaf not measured: needs server output.
- `{ ...{ __proto__: store } }` is silent in Chrome and uncertifiable here.
- `o[k] = store` with a run-time `__proto__` key is a stated premise.
- `JSON.parse("{}", reviver)` read: proved before, uncertifiable now.
- Doc: #173's title and description still carry an old note; update before the end. Remove worktree `/Users/thomas/dev/sc-worktrees.noindex/rc21-baseline`, or say so.

## Stacked PRs after #173 (do not build here)

Report 2 from Fable (`blocker` on every obligation, dispatch-veto site placement, return shapes, per-slot objects), `OffJsxPath` (arbitration 1), S1 and S2 (c1 report), C12 one-link fresh-root proof (arbitration 2, 39 sites), dropping `focus` and friends (about 1,392).

## Final order, only on the owner's word

On the final commit, from census: census, all-pairs (`scripts/fuzz/focus.mjs --smoke --cover all --cover-seed 1 --sample 0`), per-code (`make fuzz-premerge`), `make verify` once, corpus diff against #173's head with every moved finding explained, then recall; plus `make test-typefacts`, `make ir-read-lint`, `make ir-edit-fuzz`, smoke reselect (`scripts/fuzz/smoke.mjs select`; `mx-census` ids must be in its manifest), per-program diff with `pcmp.py`.

Final message: per audit finding and per Fable item "proved" or "now uncertifiable" with counts; corpus diff with the 3 SC8015 reviews; PR URL and head SHA; checks with results; deferred; remaining false cleans or wrong rules. Last line: `PR <full URL>, head <SHA>, checks and reviews <state>.`
