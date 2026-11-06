# Recall grids, early look at 775ee26d1 (stopped by the memory guard)

Head: `775ee26d1` (detached; `e40429f5e` is an ancestor, checked on the host and in the VM clone). VM clone tree `8ff54c37f` = `775ee26d1^{tree}`; release checker and `solid-typefacts` rebuilt from it in the VM at 22:26:10 / 22:26:50.
VM: Colima `sc`, 12 vCPU, restarted at 24 GiB (`free -m` total 23980). Disk before: 81 GB, 65% of inodes free.
Command: `FUZZ_VM_PROFILE=sc scripts/fuzz/vm.sh detach /mnt/lima-colima/sc/runs/grids-early-775ee26d1.log node scripts/fuzz/focus.mjs --codes all --grids mx-recall,lg-recall --split-codes --prune-cases --checker-bin <clone>/rust/target/release/solid-checker-rust --typefacts <clone>/bin/solid-typefacts --out /mnt/lima-colima/sc/runs/grids-early-775ee26d1` (recall is lean, focus.mjs's default without `--recall-full`).

## Status: incomplete. The memory guard stopped the run

- Started 22:27:04; the guard stopped it at 23:47:35, when VM used memory sampled at **22,615 MB** (limit 20,480; available 1,364 MB). It sent SIGTERM then SIGKILL to process group 2868 only. No process of the run is left.
- **The checker did not cause it.** Its summed RSS peaked at **324 MB** and `solid-typefacts` at **641 MB**. The growth was the harness:
  - The Node processes reached **12,322 MB** (the `matrix.mjs` grid process runs with `--max-old-space-size=12000`, focus.mjs's recall default).
  - The tmpfs scratch held **5,484 MB** of STORE_WRITE_OUTSIDE_SETTER's `cases/` (counted in `used` here: used + available = total).
  - Chrome took 4.7 GB.
- It happened in STORE_WRITE_OUTSIDE_SETTER's Chrome oracle phase, after all 212,004 of its programs had been judged (log: `212004/212004 programs, 1580s`, then `runtime differential … on 689 programs`). REACTIVE_WRITE_IN_OWNED_SCOPE's oracle phase showed the same jump earlier (Node 4.4 → 8.8 GB at 23:16).
- So 24 GB with a 20 GB trip cannot hold the biggest recall code with the 12 GB heap default. Options for the parent: `--heap-mb` lower, `--sample-big`, or resume from CLEANUP_IN_FORBIDDEN_SCOPE with `--codes` listing the rest.
- **Done:** 8 mx-recall codes in full, plus STORE_WRITE_OUTSIDE_SETTER's rows, all judged (`programs.partial.jsonl`; no oracle comparison, no `matrix.json`).
- **Not run:** the remaining mx-recall codes (CLEANUP_IN_FORBIDDEN_SCOPE onward: PRIMITIVE_IN_FORBIDDEN_SCOPE, PENDING_ASYNC_*, SYNC_NODE_RECEIVED_ASYNC, UNTRACKED_READ_AFTER_AWAIT, ACTION_CALLED_IN_OWNED_SCOPE, RESOLVE/UNTIL, FLUSH_*, DYNAMIC_STATIC_PROMISE, PRIMITIVE_IN_EFFECT_CALLBACK, LOADING_ON_OUTSIDE_HOLD, and any newer codes) and **all of lg-recall**. So no ASYNC row was reached yet.

## Counts per grid and code

Problems are rowProblem (scripts/fuzz/codes.mjs). Held means matched by gridStanding against `scripts/fuzz/open-defects.json` at 775ee26d1 (the same test focus.mjs uses). No row came out `open-defect-fixed`. `recall-gap`, `typed-away` and `not-fired` are not problems in rowProblem and are not reviewed here.

### mx-recall

| code | programs | outcomes | FP | FC | wrong-rule | harness | other problems | held (open-defects) | not listed |
|---|---:|---|---:|---:|---:|---:|---|---|---:|
| CONTEXT_NOT_FOUND | 4905 | agree 3107, recall-gap 1798 | 0 | 0 | 0 | 0 |  |  | 0 |
| NO_OWNER_BOUNDARY | 37660 | recall-gap 21956, agree 15704 | 0 | 0 | 0 | 0 |  |  | 0 |
| NO_OWNER_CLEANUP | 19650 | recall-gap 11090, agree 8150, not-fired 275, typed-away 135 | 0 | 0 | 0 | 0 |  |  | 0 |
| NO_OWNER_CONTEXT | 14190 | agree 7603, recall-gap 6587 | 0 | 0 | 0 | 0 |  |  | 0 |
| NO_OWNER_EFFECT | 33996 | recall-gap 18222, agree 14954, not-fired 550, typed-away 270 | 0 | 0 | 0 | 0 |  |  | 0 |
| REACTIVE_WRITE_IN_OWNED_SCOPE | 103264 | recall-gap 52384, agree 45500, typed-away 5172, not-fired 132, false-clean 76 | 0 | 76 | 0 | 0 |  |  | 76 |
| SETTLED_CLEANUP_UNOWNED | 17870 | recall-gap 10158, agree 7712 | 0 | 0 | 0 | 0 |  |  | 0 |
| STORE_WRITE_OUTSIDE_SETTER (PARTIAL) | 212004 | agree 153286, recall-gap 58484, typed-away 126, not-fired 108 | 0 | 0 | 0 | 0 |  |  | 0 |
| STRICT_READ_UNTRACKED | 82690 | recall-gap 48651, agree 30579, not-fired 2100, typed-away 1354, false-clean 6 | 0 | 6 | 0 | 0 |  | setter callback in a body arrow 6 | 0 |
| **total** | 526229 | | 0 | 82 | 0 | 0 |  | | 76 |

**lg-recall:** not started (0 programs).

Oracle (Chrome) per finished unit:

| unit | compared | behaviour | attribution | inconclusive |
|---|---:|---:|---:|---:|
| STRICT_READ_UNTRACKED | 466 | 0 | 0 | 0 |
| NO_OWNER_CLEANUP | 291 | 0 | 0 | 0 |
| NO_OWNER_EFFECT | 251 | 0 | 0 | 0 |
| NO_OWNER_BOUNDARY | 279 | 0 | 0 | 0 |
| NO_OWNER_CONTEXT | 229 | 0 | 0 | 0 |
| CONTEXT_NOT_FOUND | 83 | 0 | 0 | 0 |
| SETTLED_CLEANUP_UNOWNED | 262 | 0 | 0 | 0 |
| REACTIVE_WRITE_IN_OWNED_SCOPE | 571 | 0 | 0 | 15 |
| STORE_WRITE_OUTSIDE_SETTER | killed during the oracle | | | |

Held, as expected: STRICT_READ_UNTRACKED, 6 false cleans, item "setter callback in a body arrow".

## Rows not listed as open: 76, one cause

All are **mx-recall / REACTIVE_WRITE_IN_OWNED_SCOPE, false-clean**. Each row's detail is `{"placed":"placed","findings":[],"certified":true}`: the site was placed and the file certified with no finding, a silent clean.

- **Shape:** contexts `pos-handler-root` (38) and `pos-handler-owner` (38): a write inside `createRoot`, or `runWithOwner(boundOwner, …)`, inside an `onClick` handler. rc.17 throws on the write there (misuse.mjs).
- **Only the forward layers fail:** `forward-2` (38) and `forward-13` (38) fail for every variant (signal 28, store 28, refresh 20) and every binding (direct, alias, destructured, namespace, wrapper, hook, hook-member, tuple).
- **Every other layer holds:** in these two contexts, chain-5, chain-13, helper, closure and the rest come out `agree` or `recall-gap`, never a false clean.
- **Every other context holds:** the forward layers are `agree` or `recall-gap` in all other 50 REACTIVE_WRITE_IN_OWNED_SCOPE contexts.
- **Likely cause** (not investigated): the forward-N placement from ADR 0695 (`1787e8c7d`, 2026-11-06, which added `forward-2`/`forward-13` to CHAIN_FORMS) does not carry a root or bound owner created in a handler. It certifies the forwarded callback's write instead of placing it in the owned scope, or else leaving it uncertifiable.
- **Chrome:** 2 of the 76 were in the Chrome oracle sample with no behaviour difference: `…-pos-handler-root-store-tuple-forward-13` and `…-pos-handler-owner-store-hook-member-forward-13`. The runtime record the rows were judged against (the write throws) is confirmed there.
- **tsc:** none is `typed-away`, so tsc accepts them. By the owner's rule these are real false cleans.
- **Prior runs:** none of the earlier VM runs (`runs/r3/*`, `runs/baseline-main`) has these ids. They predate the forward layers.

- `mx-reactive-write-in-owned-scope-pos-handler-owner-refresh-alias-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-refresh-alias-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-refresh-destructured-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-refresh-destructured-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-refresh-direct-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-refresh-direct-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-refresh-namespace-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-refresh-namespace-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-refresh-wrapper-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-refresh-wrapper-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-alias-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-alias-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-destructured-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-destructured-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-direct-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-direct-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-hook-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-hook-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-hook-member-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-hook-member-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-tuple-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-tuple-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-wrapper-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-signal-wrapper-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-alias-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-alias-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-destructured-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-destructured-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-direct-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-direct-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-hook-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-hook-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-hook-member-forward-13` false-clean: certified, no finding at the placed site, layer forward-13; in the Chrome oracle sample, no behaviour difference
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-hook-member-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-tuple-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-tuple-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-wrapper-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-owner-store-wrapper-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-refresh-alias-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-refresh-alias-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-refresh-destructured-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-refresh-destructured-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-refresh-direct-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-refresh-direct-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-refresh-namespace-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-refresh-namespace-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-refresh-wrapper-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-refresh-wrapper-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-alias-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-alias-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-destructured-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-destructured-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-direct-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-direct-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-hook-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-hook-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-hook-member-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-hook-member-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-tuple-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-tuple-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-wrapper-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-signal-wrapper-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-alias-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-alias-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-destructured-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-destructured-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-direct-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-direct-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-hook-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-hook-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-hook-member-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-hook-member-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-tuple-forward-13` false-clean: certified, no finding at the placed site, layer forward-13; in the Chrome oracle sample, no behaviour difference
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-tuple-forward-2` false-clean: certified, no finding at the placed site, layer forward-2
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-wrapper-forward-13` false-clean: certified, no finding at the placed site, layer forward-13
- `mx-reactive-write-in-owned-scope-pos-handler-root-store-wrapper-forward-2` false-clean: certified, no finding at the placed site, layer forward-2

## Time, throughput, memory

| | |
|---|---|
| Wall time | 22:27:04 → 23:47:35 = **4,831 s** (80.5 min) |
| Programs judged | **526,229** (314,225 in 8 finished units + 212,004 STORE_WRITE_OUTSIDE_SETTER) |
| Throughput | **~109 programs/s** overall, including oracle phases. Finished units: 314,225 in 3,196 s = 98/s. STORE_WRITE_OUTSIDE_SETTER's evaluation: 212,004 in 1,580 s = 134/s |
| Unit times | STRICT_READ_UNTRACKED 760 s, NO_OWNER_CLEANUP 226, NO_OWNER_EFFECT 326, NO_OWNER_BOUNDARY 368, NO_OWNER_CONTEXT 177, CONTEXT_NOT_FOUND 60, SETTLED_CLEANUP_UNOWNED 207, REACTIVE_WRITE_IN_OWNED_SCOPE 1,072, STORE_WRITE_OUTSIDE_SETTER ~1,650 until killed |
| Peak VM memory (`free -m` used, 40 s samples) | **22,615 MB** at the trip; 20,067 MB at 23:46:14; before STORE_WRITE_OUTSIDE_SETTER's oracle phase ≤ 16.7 GB |
| Peak RSS by process type | Node 12,322 MB, Chrome 5,925 MB, tmpfs shared 5,484 MB, solid-typefacts 641 MB, solid-checker 324 MB |

Grid sizes grew since the 2026-11-01 baseline-main run (lean recall): STRICT_READ_UNTRACKED 8,748 → 82,690; REACTIVE_WRITE_IN_OWNED_SCOPE 53,808 → 103,264; STORE_WRITE_OUTSIDE_SETTER 89,790 → 212,004.

## Where things are

- **VM:** `/mnt/lima-colima/sc/runs/grids-early-775ee26d1{,.log}`. STORE_WRITE_OUTSIDE_SETTER's `cases/` and `chrome-cases/` are still on the tmpfs scratch (`/mnt/lima-colima/sc/ram/scratch`, 5.4 GB), not pruned.
- **Host copy** (without cases): `~/EDF/tmp/flipw/grids-early/run/`.
- **Analysis:** `~/EDF/tmp/flipw/grids-early/analysis.json` (every not-listed row with its full row fields), `tables.md`, and the guard's samples in `mem.log`.
- **Scripts:** `analyze.mjs`, `tables.mjs`, `guard.sh`, all beside them.
- **Side effect:** restarting `sc` set the host's docker context to `colima-sc`.
