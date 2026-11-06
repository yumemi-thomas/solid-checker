# Recall grids at 4c81fcc00: every mx-recall and lg-recall code, in full

Head: `4c81fcc00` (claude/flip-census tip, fetched 2026-11-07 about 02:33 JST), in a new detached worktree. `git merge-base --is-ancestor e40429f5e 4c81fcc00` holds, and `rust/crates/solid-facts/src/ast/types.rs:2165` has `if !seen.insert(span)` inside `composite_callee_walk` (2130). The VM clone was synced with `FUZZ_VM_PROFILE=sc scripts/fuzz/vm.sh sync`: sync commit `aa01e86c2`, parent `4c81fcc00`, tree `494e58b4c` (the same as `4c81fcc00^{tree}`), and the fix is an ancestor there too. Release `solid-typefacts` and `solid-checker-rust` were built in the VM from that tree at 02:36:20 and 02:36:55 (sha256 `c67df9f6…`, `b3c8fedb…`). They were the same at the start and end of every stage, and the clone's HEAD did not move. No other binary was run.
VM: Colima `sc`, 12 vCPU, 24 GiB (23,980 MB total), already running, configuration unchanged. The memory guard was my own (`stage.sh`). It sampled every 10 s, with a limit of 20,480 MB `used`, and could kill only its own stage's process group. **It never tripped.**

## Commands

Two stages, run detached (`setsid nohup`) through `plan.sh`. Both took `--grids mx-recall,lg-recall --split-codes --prune-cases --heap-mb 6000 --checker-bin <clone>/rust/target/release/solid-checker-rust --typefacts <clone>/bin/solid-typefacts`. Recall was lean, focus.mjs's default.
- **A** (02:37:51 → 04:27:04): `--codes` listing the 29 recall codes other than STORE_WRITE_OUTSIDE_SETTER.
- **B** (04:27:05 → 04:40:58): `--codes STORE_WRITE_OUTSIDE_SETTER`. It ran alone and last, so a trip could cost only that code.
- **C** (confirmation, 04:42 → 04:46): every not-listed problem id of A, by id (`rerun-ids.mjs` manifests), through `matrix.mjs`/`legal.mjs --confirm-with-chrome --runtime-diff all`, with no per-cell cap on false cleans. It ran twice, the second time with `--no-chrome-cache`, which is the pass reported below. Cases were kept so the programs could be read.

## Status: complete. Nothing sampled, nothing skipped

- **Every code ran in full**: all 30 mx-recall codes (681,839 programs) and all 28 lg-recall codes (394,549 programs), **1,076,388 programs**. No `--sample-big` was needed, and no unit is partial, sampled or missing.
- No harness rows, no tsc-overlap, no secondary false cleans, no not-legal rows. The Chrome oracle compared 8,748 programs (4,722 mx, 4,026 lg) with **0 behaviour and 0 attribution differences**. 15 were inconclusive, all in mx REACTIVE_WRITE_IN_OWNED_SCOPE (the same count as the previous run).
- focus.mjs exited 1 for both A and B, which it does whenever anything needs a look. These are the rows below.
- lg STRICT_READ_UNTRACKED has 5 `disturbed` rows (`dny-blur-lowercase`). Chrome raised LOWERCASE_EVENT_ATTRIBUTE, which could not be placed on a line, and the checker was clean. rowProblem does not count these, and they were not judged.

## Counts per grid and code

FP, FC and wrong-rule come from rowProblem (scripts/fuzz/codes.mjs). "held" means matched by gridStanding against `scripts/fuzz/open-defects.json` at 4c81fcc00, the test focus.mjs uses. The oracle column is the unit's Chrome sample: compared/behaviour/attribution/inconclusive.

### mx-recall

| code | programs | outcomes | FP | FC | wrong-rule | harness | held | not listed | Chrome oracle (compared/behaviour/attribution/inconclusive) | s |
|---|---:|---|---:|---:|---:|---:|---|---:|---|---:|
| ACTION_CALLED_IN_OWNED_SCOPE | 29940 | agree 16190, recall-gap 12598, typed-away 1152 | 0 | 0 | 0 | 0 |  | 0 | 429/0/0/0 | 337 |
| ASYNC_OUTSIDE_LOADING_BOUNDARY | 4662 | recall-gap 3808, agree 480, not-fired 366, false-clean 8 | 0 | 8 | 0 | 0 |  | 8 | 66/0/0/0 | 52 |
| CAUGHT_REACTIVE_READ | 1112 | recall-gap 784, agree 328 | 0 | 0 | 0 | 0 |  | 0 | 24/0/0/0 | 16 |
| CLEANUP_IN_FORBIDDEN_SCOPE | 2590 | recall-gap 1678, agree 912 | 0 | 0 | 0 | 0 |  | 0 | 55/0/0/0 | 36 |
| CONTEXT_NOT_FOUND | 4905 | agree 3107, recall-gap 1798 | 0 | 0 | 0 | 0 |  | 0 | 83/0/0/0 | 20 |
| DYNAMIC_STATIC_PROMISE | 6300 | agree 4818, recall-gap 1482 | 0 | 0 | 0 | 0 |  | 0 | 83/0/0/0 | 67 |
| FLUSH_IN_ACTION | 1590 | recall-gap 914, agree 516, typed-away 140, false-clean 20 | 0 | 20 | 0 | 0 |  | 20 | 24/0/0/0 | 20 |
| FLUSH_IN_EFFECT_CALLBACK | 6058 | recall-gap 4411, agree 1512, typed-away 135 | 0 | 0 | 0 | 0 |  | 0 | 91/0/0/0 | 67 |
| FLUSH_IN_TRACKED_QUEUE_CALLBACK | 2160 | recall-gap 1362, agree 798 | 0 | 0 | 0 | 0 |  | 0 | 45/0/0/0 | 29 |
| LOADING_ON_ACCESSOR | 12222 | recall-gap 8946, agree 3276 | 0 | 0 | 0 | 0 |  | 0 | 49/0/0/0 | 101 |
| LOADING_ON_OUTSIDE_HOLD | 582 | recall-gap 582 | 0 | 0 | 0 | 0 |  | 0 | 7/0/0/0 | 9 |
| MISSING_EFFECT_FN | 2940 | agree 2400, recall-gap 360, typed-away 180 | 0 | 0 | 0 | 0 |  | 0 | 21/0/0/0 | 29 |
| NO_OWNER_BOUNDARY | 37660 | recall-gap 21956, agree 15704 | 0 | 0 | 0 | 0 |  | 0 | 279/0/0/0 | 134 |
| NO_OWNER_CLEANUP | 19650 | recall-gap 11090, agree 8150, not-fired 275, typed-away 135 | 0 | 0 | 0 | 0 |  | 0 | 291/0/0/0 | 77 |
| NO_OWNER_CONTEXT | 14190 | agree 7603, recall-gap 6587 | 0 | 0 | 0 | 0 |  | 0 | 229/0/0/0 | 57 |
| NO_OWNER_EFFECT | 33996 | recall-gap 18222, agree 14954, not-fired 550, typed-away 270 | 0 | 0 | 0 | 0 |  | 0 | 251/0/0/0 | 121 |
| ON_FLUSH_IN_OWNED_SCOPE | 2650 | agree 1892, recall-gap 758 | 0 | 0 | 0 | 0 |  | 0 | 32/0/0/0 | 31 |
| OPTIMISTIC_WRITE_BEFORE_LOAD | 2724 | agree 1378, recall-gap 1346 | 0 | 0 | 0 | 0 |  | 0 | 17/0/0/0 | 28 |
| PENDING_ASYNC_FORBIDDEN_SCOPE | 2790 | agree 2100, recall-gap 687, false-clean 3 | 0 | 3 | 0 | 0 |  | 3 | 50/0/0/0 | 34 |
| PENDING_ASYNC_UNTRACKED_READ | 14098 | recall-gap 9842, agree 4034, not-fired 210, wrong-rule 12 | 0 | 0 | 12 | 0 |  | 12 | 181/0/0/0 | 151 |
| PRIMITIVE_IN_EFFECT_CALLBACK | 12600 | recall-gap 6864, agree 4101, typed-away 1635 | 0 | 0 | 0 | 0 |  | 0 | 50/0/0/0 | 113 |
| PRIMITIVE_IN_FORBIDDEN_SCOPE | 10410 | recall-gap 7530, agree 2880 | 0 | 0 | 0 | 0 |  | 0 | 65/0/0/0 | 95 |
| REACTIVE_WRITE_IN_OWNED_SCOPE | 103372 | recall-gap 52530, agree 45538, typed-away 5172, not-fired 132 | 0 | 0 | 0 | 0 |  | 0 | 569/0/0/15 | 430 |
| RESOLVE_IN_REACTIVE_SCOPE | 1455 | recall-gap 729, agree 726 | 0 | 0 | 0 | 0 |  | 0 | 17/0/0/0 | 18 |
| SETTLED_CLEANUP_UNOWNED | 17870 | recall-gap 10158, agree 7712 | 0 | 0 | 0 | 0 |  | 0 | 262/0/0/0 | 70 |
| STRICT_READ_UNTRACKED | 84922 | recall-gap 50715, agree 30651, not-fired 2190, typed-away 1360, false-clean 6 | 0 | 6 | 0 | 0 |  | 6 | 467/0/0/0 | 270 |
| SYNC_NODE_RECEIVED_ASYNC | 21700 | agree 17544, recall-gap 4156 | 0 | 0 | 0 | 0 |  | 0 | 138/0/0/0 | 193 |
| UNTIL_IN_REACTIVE_SCOPE | 1455 | recall-gap 729, agree 726 | 0 | 0 | 0 | 0 |  | 0 | 17/0/0/0 | 19 |
| UNTRACKED_READ_AFTER_AWAIT | 13088 | recall-gap 9754, agree 3264, false-clean 70 | 0 | 70 | 0 | 0 |  | 70 | 141/0/0/0 | 134 |
| STORE_WRITE_OUTSIDE_SETTER | 212148 | agree 153322, recall-gap 58592, typed-away 126, not-fired 108 | 0 | 0 | 0 | 0 |  | 0 | 689/0/0/0 | 769 |
| **total (30 codes)** | 681839 | | 0 | 107 | 12 | 0 | | 119 | 4722/0/0/15 | 3527 |

### lg-recall

| code | programs | outcomes | FP | FC | wrong-rule | harness | held | not listed | Chrome oracle (compared/behaviour/attribution/inconclusive) | s |
|---|---:|---|---:|---:|---:|---:|---|---:|---|---:|
| ACTION_CALLED_IN_OWNED_SCOPE | 33488 | uncertifiable 20559, clean 12929 | 0 | 0 | 0 | 0 |  | 0 | 459/0/0/0 | 363 |
| ASYNC_OUTSIDE_LOADING_BOUNDARY | 3334 | uncertifiable 2746, clean 588 | 0 | 0 | 0 | 0 |  | 0 | 81/0/0/0 | 40 |
| CAUGHT_REACTIVE_READ | 4860 | uncertifiable 3472, clean 1388 | 0 | 0 | 0 | 0 |  | 0 | 28/0/0/0 | 41 |
| CLEANUP_IN_FORBIDDEN_SCOPE | 1930 | clean 1124, uncertifiable 806 | 0 | 0 | 0 | 0 |  | 0 | 46/0/0/0 | 24 |
| CONTEXT_NOT_FOUND | 330 | uncertifiable 300, clean 30 | 0 | 0 | 0 | 0 |  | 0 | 11/0/0/0 | 8 |
| DYNAMIC_STATIC_PROMISE | 180 | uncertifiable 150, clean 30 | 0 | 0 | 0 | 0 |  | 0 | 6/0/0/0 | 5 |
| FLUSH_IN_ACTION | 8250 | clean 6042, uncertifiable 2208 | 0 | 0 | 0 | 0 |  | 0 | 127/0/0/0 | 87 |
| FLUSH_IN_EFFECT_CALLBACK | 12770 | clean 7971, uncertifiable 4799 | 0 | 0 | 0 | 0 |  | 0 | 211/0/0/0 | 133 |
| FLUSH_IN_TRACKED_QUEUE_CALLBACK | 9000 | clean 6732, uncertifiable 2268 | 0 | 0 | 0 | 0 |  | 0 | 157/0/0/0 | 88 |
| LOADING_ON_ACCESSOR | 582 | clean 450, uncertifiable 132 | 0 | 0 | 0 | 0 |  | 0 | 13/0/0/0 | 10 |
| NO_OWNER_BOUNDARY | 3560 | uncertifiable 2170, clean 1390 | 0 | 0 | 0 | 0 |  | 0 | 44/0/0/0 | 39 |
| NO_OWNER_CLEANUP | 11727 | uncertifiable 5959, clean 5768 | 0 | 0 | 0 | 0 |  | 0 | 195/0/0/0 | 132 |
| NO_OWNER_CONTEXT | 5835 | uncertifiable 3781, clean 2054 | 0 | 0 | 0 | 0 |  | 0 | 105/0/0/0 | 71 |
| NO_OWNER_EFFECT | 16134 | clean 8380, uncertifiable 7754 | 0 | 0 | 0 | 0 |  | 0 | 157/0/0/0 | 151 |
| ON_FLUSH_IN_OWNED_SCOPE | 2090 | clean 1701, uncertifiable 389 | 0 | 0 | 0 | 0 |  | 0 | 24/0/0/0 | 22 |
| OPTIMISTIC_WRITE_BEFORE_LOAD | 1200 | uncertifiable 1152, clean 48 | 0 | 0 | 0 | 0 |  | 0 | 9/0/0/0 | 13 |
| PENDING_ASYNC_FORBIDDEN_SCOPE | 5616 | uncertifiable 3947, clean 1668, false-positive 1 | 1 | 0 | 0 | 0 |  | 1 | 125/0/0/0 | 62 |
| PENDING_ASYNC_UNTRACKED_READ | 11371 | uncertifiable 9547, clean 1824 | 0 | 0 | 0 | 0 |  | 0 | 193/0/0/0 | 131 |
| PRIMITIVE_IN_EFFECT_CALLBACK | 12880 | clean 6516, uncertifiable 6364 | 0 | 0 | 0 | 0 |  | 0 | 108/0/0/0 | 110 |
| PRIMITIVE_IN_FORBIDDEN_SCOPE | 5910 | clean 4272, uncertifiable 1638 | 0 | 0 | 0 | 0 |  | 0 | 60/0/0/0 | 55 |
| REACTIVE_WRITE_IN_OWNED_SCOPE | 118865 | uncertifiable 82154, clean 36711 | 0 | 0 | 0 | 0 |  | 0 | 800/0/0/0 | 1085 |
| RESOLVE_IN_REACTIVE_SCOPE | 8430 | uncertifiable 6249, clean 2181 | 0 | 0 | 0 | 0 |  | 0 | 123/0/0/0 | 95 |
| SETTLED_CLEANUP_UNOWNED | 2750 | clean 1494, uncertifiable 1256 | 0 | 0 | 0 | 0 |  | 0 | 58/0/0/0 | 37 |
| STRICT_READ_UNTRACKED | 91573 | uncertifiable 52464, clean 37466, typed-away 1638, disturbed 5 | 0 | 0 | 0 | 0 |  | 0 | 596/0/0/0 | 825 |
| SYNC_NODE_RECEIVED_ASYNC | 360 | uncertifiable 300, clean 60 | 0 | 0 | 0 | 0 |  | 0 | 6/0/0/0 | 6 |
| UNTIL_IN_REACTIVE_SCOPE | 8430 | uncertifiable 6249, clean 2181 | 0 | 0 | 0 | 0 |  | 0 | 123/0/0/0 | 93 |
| UNTRACKED_READ_AFTER_AWAIT | 5642 | uncertifiable 2825, clean 2817 | 0 | 0 | 0 | 0 |  | 0 | 103/0/0/0 | 59 |
| STORE_WRITE_OUTSIDE_SETTER | 7452 | clean 5098, uncertifiable 2336, typed-away 18 | 0 | 0 | 0 | 0 |  | 720 (open-defect-fixed) | 58/0/0/0 | 58 |
| **total (28 codes)** | 394549 | | 1 | 0 | 0 | 0 | | 721 | 4026/0/0/0 | 3843 |

## Against open-defects.json at 4c81fcc00

That list holds three grid entries. **None of them held a row in this run.**
- STRICT_READ_UNTRACKED "ref in a control-flow callback" and "layer inside a named ref" (`store-dstr-*`, contexts ref-row, ref-named, ref-factory): all 216 programs are now `recall-gap` (uncertifiable at the site), and none is a false clean. Both entries are `partial`, so the gate does not flag them as fixed. They no longer reproduce.
- lg STORE_WRITE_OUTSIDE_SETTER "V-D1" (`noop-*`): **720 rows `open-defect-fixed`**, now clean 234 and uncertifiable 486. Per open-defects.mjs, the entry has to move to rediscovery.json, or every full run fails on these 720.

**The list lost 8 grid entries in a merge.** `f1d5f3710` ("Merge branch 'claude/flip-r1' into claude/flip-census", 2026-11-06 20:33, conflict in open-defects.json) went from the 11 grid entries of its first parent `dd137e284` down to 4, and later merges kept it that way. Two of the dropped entries hold rows of this run: STRICT_READ_UNTRACKED "setter callback in a body arrow" (6 rows; docs/open-work.md:29 still says "Held in open-defects.json") and ASYNC_OUTSIDE_LOADING_BOUNDARY "census" (8 rows, a `partial` entry with no context restriction). The other six dropped entries (UNTRACKED_READ_AFTER_AWAIT then-named; PENDING_ASYNC_* member-tag getter; RESOLVE/UNTIL/CAUGHT handed-flush; CAUGHT_REACTIVE_READ mount-named) held nothing here. UNTRACKED_READ_AFTER_AWAIT then-named is 676 recall gaps, none failing. The handed-flush, mount-named and async-value-member shapes have no rows in the recall grids (they are mx-src/mx-base shapes, not run here). The column "held before f1d5f3710" below uses dd137e284's list for comparison only.

## Rows not listed as open: 120 (107 false clean, 12 wrong rule, 1 false positive)

Each one was run again by id in real Chrome without the cache (stage C). **Chrome reproduced every one**: all 119 mx rows are `chrome.status: same` (the program misbehaves in Chrome as the record says, and the checker's outcome stands), and the lg false positive is `confirmed` (Chrome runs it cleanly). None is `typed-away`, so tsc accepts every one. "Silent" below means certified: the file has no uncertifiable finding anywhere. That is the clean the owner's rule forbids. The other false cleans have an uncertifiable at or near the site, but from a rule that does not claim the code.

| grid/code | rows | silent (certified) | uncertifiable, other rule | shape | held before f1d5f3710 |
|---|---:|---:|---:|---|---|
| mx FLUSH_IN_ACTION | 20 | **20** | 0 | `flush()` in a callback forwarded through 2 or 13 `(g) => { hcN(g); }` wrappers inside `action(function* …)`, before or after a `yield`; every binding. Only the forward-2/forward-13 layers fail: chain-5/13, depth-15 and the handoffs agree or are gaps | no |
| mx STRICT_READ_UNTRACKED | 6 | **6** | 0 | `const wrapUse = () => { setSw2((d) => { d.n = a1(); }); }` called in an effect's apply (`setter-fn-apply`, `wrapper` binding, layers none, closure, helper, iife, closure+helper, module-helper). Known: open-work.md:29 | yes ("setter callback in a body arrow") |
| mx ASYNC_OUTSIDE_LOADING_BOUNDARY | 8 | **2** | 6 | `const wrapUse = () => { a1(); }` (an async memo) read in tracked JSX through the `wrapper` binding. In the `for` context (chain-5, chain-13) the file is certified. In jsx, show and portal-jsx (forward-2, forward-13) the site has `strict-read-untracked` and `pending-async-unsuspendable-read` uncertifiables. Chrome reports the code unplaced | yes ("census") |
| mx PENDING_ASYNC_FORBIDDEN_SCOPE | 3 | **1** | 2 | `wrapUse` (async memo read) through chain-5 (`hc4 → … → hc0 → wrapUse()`), `wrapper` binding only: `onSettled` is certified; `createTrackedEffect`, tracked and tracked-rerun, have an `async-outside-loading-boundary` uncertifiable at the site. The other 6 bindings agree at chain-5 | no (the dropped entry was the async-value-member source) |
| mx UNTRACKED_READ_AFTER_AWAIT | 70 | 0 | 70 | A read after `yield 1` in a generator inside an async memo: `async function*` with `for await` (gen-yield), or a sync generator with `await` in the loop (gen-sync-loop). Reached through handoff-member (`ho.m()`), handoff-assign, handoff-assign-param, forward-2 or forward-13, with every binding. The site has `strict-read-untracked/uncertifiable`, never the claiming rule. Every other layer of these two contexts is a recall gap | no |
| mx PENDING_ASYNC_UNTRACKED_READ | 12 (wrong rule) | | | `<Show when={1}>{(_u) => { hc1(() => { a1(); }); … }}</Show>` (async memo, forward-2/forward-13; every binding but `wrapper`, which is a recall gap). The checker reports `async-outside-loading-boundary` **violation** (SC5003) at the read, and Chrome reports only PENDING_ASYNC_UNTRACKED_READ there: a violation the runtime does not corroborate, and the claiming rule's miss. Every other layer of flow-show agrees or is a gap | no |
| lg PENDING_ASYNC_FORBIDDEN_SCOPE | 1 (**false positive**) | | | `onSettled(() => { const hf = () => { a1(); }; const xs = JSON.parse("[]") as number[]; xs.forEach(hf); })`. The array is empty, so `hf` never runs. The checker reports `pending-async-unsuspendable-read` **violation** and an uncertifiable, both at line 8. Chrome confirms the program runs legally. Invariant 2 | no |

Note on layers: forward-2/forward-13 (ADR 0695, `1787e8c7d`) again account for most rows: 20 + 6 + 40 + 12 = 78 of 120. This is the same layer family as the previous run's 76 REACTIVE_WRITE_IN_OWNED_SCOPE false cleans. **Those 76 are gone at 4c81fcc00**: 38 agree and 38 are recall gaps.

### Every row, with its uncached Chrome verdict

#### mx-recall/ASYNC_OUTSIDE_LOADING_BOUNDARY (8)

- `mx-async-outside-loading-boundary-jsx-x-wrapper-forward-2` false-clean; unplaced; not certified: strict-read-untracked/uncertifiable@13, pending-async-unsuspendable-read/uncertifiable@13; Chrome (uncached): same, false-clean; held before f1d5f3710 as "census"
- `mx-async-outside-loading-boundary-jsx-x-wrapper-forward-13` false-clean; unplaced; not certified: strict-read-untracked/uncertifiable@24, pending-async-unsuspendable-read/uncertifiable@24; Chrome (uncached): same, false-clean; held before f1d5f3710 as "census"
- `mx-async-outside-loading-boundary-show-x-wrapper-forward-2` false-clean; unplaced; not certified: strict-read-untracked/uncertifiable@14, pending-async-unsuspendable-read/uncertifiable@14; Chrome (uncached): same, false-clean; held before f1d5f3710 as "census"
- `mx-async-outside-loading-boundary-show-x-wrapper-forward-13` false-clean; unplaced; not certified: strict-read-untracked/uncertifiable@25, pending-async-unsuspendable-read/uncertifiable@25; Chrome (uncached): same, false-clean; held before f1d5f3710 as "census"
- `mx-async-outside-loading-boundary-for-x-wrapper-chain-5` false-clean; unplaced; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean; held before f1d5f3710 as "census"
- `mx-async-outside-loading-boundary-for-x-wrapper-chain-13` false-clean; unplaced; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean; held before f1d5f3710 as "census"
- `mx-async-outside-loading-boundary-portal-jsx-x-wrapper-forward-2` false-clean; unplaced; not certified: strict-read-untracked/uncertifiable@13, pending-async-unsuspendable-read/uncertifiable@13; Chrome (uncached): same, false-clean; held before f1d5f3710 as "census"
- `mx-async-outside-loading-boundary-portal-jsx-x-wrapper-forward-13` false-clean; unplaced; not certified: strict-read-untracked/uncertifiable@24, pending-async-unsuspendable-read/uncertifiable@24; Chrome (uncached): same, false-clean; held before f1d5f3710 as "census"

#### mx-recall/FLUSH_IN_ACTION (20)

- `mx-flush-in-action-action-x-direct-forward-2` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-x-direct-forward-13` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-x-namespace-forward-2` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-x-namespace-forward-13` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-x-alias-forward-2` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-x-alias-forward-13` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-x-destructured-forward-2` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-x-destructured-forward-13` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-x-wrapper-forward-2` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-x-wrapper-forward-13` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-after-yield-x-direct-forward-2` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-after-yield-x-direct-forward-13` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-after-yield-x-namespace-forward-2` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-after-yield-x-namespace-forward-13` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-after-yield-x-alias-forward-2` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-after-yield-x-alias-forward-13` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-after-yield-x-destructured-forward-2` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-after-yield-x-destructured-forward-13` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-after-yield-x-wrapper-forward-2` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-flush-in-action-action-after-yield-x-wrapper-forward-13` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean

#### mx-recall/PENDING_ASYNC_FORBIDDEN_SCOPE (3)

- `mx-pending-async-forbidden-scope-settled-x-wrapper-chain-5` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean
- `mx-pending-async-forbidden-scope-tracked-x-wrapper-chain-5` false-clean; placed; not certified: async-outside-loading-boundary/uncertifiable@9; Chrome (uncached): same, false-clean
- `mx-pending-async-forbidden-scope-tracked-rerun-x-wrapper-chain-5` false-clean; placed; not certified: async-outside-loading-boundary/uncertifiable@11; Chrome (uncached): same, false-clean

#### mx-recall/PENDING_ASYNC_UNTRACKED_READ (12)

- `mx-pending-async-untracked-read-flow-show-x-direct-forward-2` wrong-rule; placed; async-outside-loading-boundary/violation@13; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-direct-forward-13` wrong-rule; placed; async-outside-loading-boundary/violation@24; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-alias-forward-2` wrong-rule; placed; async-outside-loading-boundary/violation@14; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-alias-forward-13` wrong-rule; placed; async-outside-loading-boundary/violation@25; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-destructured-forward-2` wrong-rule; placed; async-outside-loading-boundary/violation@14; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-destructured-forward-13` wrong-rule; placed; async-outside-loading-boundary/violation@25; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-hook-forward-2` wrong-rule; placed; async-outside-loading-boundary/violation@15, async-outside-loading-boundary/violation@15; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-hook-forward-13` wrong-rule; placed; async-outside-loading-boundary/violation@26, async-outside-loading-boundary/violation@26; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-hook-member-forward-2` wrong-rule; placed; async-outside-loading-boundary/violation@15; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-hook-member-forward-13` wrong-rule; placed; async-outside-loading-boundary/violation@26; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-tuple-forward-2` wrong-rule; placed; async-outside-loading-boundary/violation@15; Chrome (uncached): same, wrong-rule
- `mx-pending-async-untracked-read-flow-show-x-tuple-forward-13` wrong-rule; placed; async-outside-loading-boundary/violation@26; Chrome (uncached): same, wrong-rule

#### mx-recall/STRICT_READ_UNTRACKED (6)

- `mx-strict-read-untracked-effect-apply-setter-fn-apply-wrapper-none` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean; held before f1d5f3710 as "setter callback in a body arrow"
- `mx-strict-read-untracked-effect-apply-setter-fn-apply-wrapper-closure` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean; held before f1d5f3710 as "setter callback in a body arrow"
- `mx-strict-read-untracked-effect-apply-setter-fn-apply-wrapper-helper` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean; held before f1d5f3710 as "setter callback in a body arrow"
- `mx-strict-read-untracked-effect-apply-setter-fn-apply-wrapper-iife` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean; held before f1d5f3710 as "setter callback in a body arrow"
- `mx-strict-read-untracked-effect-apply-setter-fn-apply-wrapper-closure-helper` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean; held before f1d5f3710 as "setter callback in a body arrow"
- `mx-strict-read-untracked-effect-apply-setter-fn-apply-wrapper-module-helper` false-clean; placed; certified, no uncertifiable in the file; Chrome (uncached): same, false-clean; held before f1d5f3710 as "setter callback in a body arrow"

#### mx-recall/UNTRACKED_READ_AFTER_AWAIT (70)

- `mx-untracked-read-after-await-gen-yield-x-direct-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@10; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-direct-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@11; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-direct-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@11; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-direct-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-direct-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@23; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-alias-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@11; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-alias-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-alias-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-alias-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-alias-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@24; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-destructured-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@11; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-destructured-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-destructured-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-destructured-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-destructured-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@24; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-wrapper-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@5, strict-read-untracked/uncertifiable@11; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-wrapper-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@5, strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-wrapper-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@5, strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-wrapper-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-wrapper-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@24; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-hook-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-hook-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-hook-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-hook-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@14; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-hook-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@25; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-hook-member-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12, strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-hook-member-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13, strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-hook-member-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13, strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-hook-member-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@14, strict-read-untracked/uncertifiable@14; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-hook-member-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@25, strict-read-untracked/uncertifiable@25; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-tuple-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-tuple-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-tuple-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-tuple-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@14; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-yield-x-tuple-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@25; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-direct-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@10; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-direct-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@11; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-direct-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@11; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-direct-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-direct-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@23; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-alias-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@11; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-alias-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-alias-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-alias-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-alias-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@24; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-destructured-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@11; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-destructured-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-destructured-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-destructured-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-destructured-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@24; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-wrapper-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@5, strict-read-untracked/uncertifiable@11; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-wrapper-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@5, strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-wrapper-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@5, strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-wrapper-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-wrapper-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@24; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-hook-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-hook-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-hook-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-hook-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@14; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-hook-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@25; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-hook-member-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12, strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-hook-member-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13, strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-hook-member-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13, strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-hook-member-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@14, strict-read-untracked/uncertifiable@14; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-hook-member-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@25, strict-read-untracked/uncertifiable@25; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-tuple-handoff-member` false-clean; placed; not certified: strict-read-untracked/uncertifiable@12; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-tuple-handoff-assign` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-tuple-handoff-assign-param` false-clean; placed; not certified: strict-read-untracked/uncertifiable@13; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-tuple-forward-2` false-clean; placed; not certified: strict-read-untracked/uncertifiable@14; Chrome (uncached): same, false-clean
- `mx-untracked-read-after-await-gen-sync-loop-x-tuple-forward-13` false-clean; placed; not certified: strict-read-untracked/uncertifiable@25; Chrome (uncached): same, false-clean

#### lg-recall/PENDING_ASYNC_FORBIDDEN_SCOPE (1)

- `lg-pending-async-forbidden-scope-zero-settled-var-x-direct-none` false-positive; -; pending-async-unsuspendable-read/uncertifiable@8, pending-async-unsuspendable-read/violation@8; Chrome (uncached): confirmed, false-positive

## Memory, time, throughput

| | |
|---|---|
| Wall time | A 6,553 s (109 min), B 833 s (14 min), C about 4 min; A + B **7,386 s** |
| Programs | 1,076,388 judged, **146 programs/s** overall including the oracle phases (mx STORE_WRITE_OUTSIDE_SETTER alone: 212,148 in 769 s = 276/s) |
| Peak VM `used` (10 s samples) | A **19,385 MB** at 04:01:28 (lg REACTIVE_WRITE_IN_OWNED_SCOPE's oracle phase); B **19,531 MB** at 04:38:48 (mx STORE_WRITE_OUTSIDE_SETTER's oracle phase). Both are under the 20,480 limit, by 1.1 and 0.9 GB |
| Peak by process type | **solid-checker: summed RSS 349 MB, the largest single process 25 MB**; solid-typefacts summed 711 MB; Node summed 11,070 MB (the largest single Node process 5,552 MB; the grid processes ran with `--max-old-space-size=6000`); Chrome 7,411 MB; tmpfs cases 5,506 MB (B) |
| Checker per program | not measured: every check went to a warm daemon (`cost.json` `unmeasured: warm daemon`). mx p50 and p99 per unit are in each `cost.json` |

With the 6000 MB heap no single Node process passed 5.6 GB. The previous run reached 12.3 GB of Node on the same code. That made the difference against the 22.6 GB trip, but 0.9 GB of headroom is thin. A larger STORE_WRITE_OUTSIDE_SETTER grid would need `--sample-big`, or a heap below 6000.

## Where things are

- **VM**: `/mnt/lima-colima/sc/runs/grids-merged-4c81fcc00/` holds `A/`, `B/` (cases pruned), `confirm-A/`, `confirm-A-fresh/` (cases kept, 120 programs), `confirm-manifests-A/`, the logs, `*.mem` (the guard's samples), `*.status`, `analysis.json` (755 MB in all).
- **Host**: `~/EDF/tmp/flipw/grids-merged/` holds `stage.sh`, `plan.sh`, `confirm.sh`, `confirm-fresh.sh`, `analyze.mjs` (adapted from grids-early, with the oracle-sample and pre-merge-list columns; `ids.mjs` adds the uncached Chrome verdicts), `group.mjs`, `tables.mjs`, `ids.mjs`, `open-defects-dd137e284.json`, and `pulled/` (analysis, logs, the confirmation rows, every unit's matrix.json/legal.json and runtime-diff.json, 50 MB).
- **Worktrees** (new, mine): `~/EDF/tmp/flipw/grids-merged-wt` (4c81fcc00, detached) and `~/EDF/tmp/flipw/grids-merged-report-wt` (this commit).

## Side effects

- `vm.sh sync` moved the VM clone `/mnt/lima-colima/sc/solid-checker` to the sync of 4c81fcc00 and rebuilt its release binaries. It also moved the host repository's `refs/fuzz-vm/sync` (vm.sh's own ref) to `aa01e86c2`.
- The VM data disk's used inodes went from 4.85 M (50%) to 6.29 M (64%) during the run, and its used space from 66 to 73 GB. Most of that is the shared runtime cache under `/mnt/lima-colima/sc/tmp/` (vm.sh's design), which I did not delete. My tmpfs scratch (`ram/scratch/focus-*`) was pruned by `--prune-cases` and is empty. No process of the run is left in the VM.
- Nothing else was touched: no other worktree, run, VM directory, profile or setting. No product code changed. No commit except this report.

## Not done

- No fix, open-defects.json or rediscovery.json change, and no fuzz-focus rediscovery for the 120 rows (this is a report).
- The checker's per-program peak RSS was not measured (warm daemons). The figures above are 10 s samples of every `solid-checker*` process.
