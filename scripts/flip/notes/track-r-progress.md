# Track R progress

Branch `claude/flip-reads`. One line per rule; fresh agents continue from here.

Helpers (this machine, scratch): `/tmp/sc-scratch.noindex/flip-reads`: `ff.sh <name> <bindir> <codes> --grids ...` (focused run), `sw.sh <tag> <prev>` (sweep build `r<tag>`, compare), `redisc.py` (append a rediscovery entry), `base/` (build at the branch cut), `sweep-base.json` (16,036 U, 383 V).

| Rule | State | Corpus move | Notes |
|---|---|---|---|
| SC5007 | done, ADR 0650 | 0 | old grid 9,540 false cleans, new 0; also a possibly undefined accessor is no longer a violation |
| SC8003 | done, ADR 0651 | +224 U, 0 V moved | witnesses 8 of 8; every added site is an open spread on an intrinsic; `outside` in rediscovery: no runtime code; also drops the old false V on `innerHTML` beside a blank child |
| SC1005 | done, ADR 0652 | +1 U (`!user` from `useContext`), 0 V moved | witnesses 9 of 9; `outside`: no runtime code |
| SC1007 | done, ADR 0653 | +41 U, 0 V moved | witnesses 3 of 3; every site a handler read from a hook's result; `outside`: the grid judges the site statement, not an attribute. Follow-up proof: a project hook returning a closed literal of functions is a plain object, which should recover most of the 41 |
| SC2007 | done, ADR 0654 | 0 | witnesses 9 of 9; old grid 1,500 false cleans, new 0; adds `return_open` (protocol 89), which SC7002 and SC5003 read |
| SC7002 | done, ADR 0655 | +1 U (`createSignal(load, {...options})`, generic return), 0 V moved | witnesses 7 of 7; old grid 1,140 false cleans, new 0 |
| SC1004 | done, ADR 0656 | -30 SC1001 U, +29 SC1004 U, same sites, 0 V moved | witnesses 2 of 2; `outside` |
| SC1003 | done, ADR 0657 | +129 U, 0 V moved | witnesses 9 of 10, `o12` waits for the census as the plan says; old grid 144 false cleans, new 0; 3 V-D5 patterns left the open list, 390 open |

| SC5008 | done, ADR 0658 | +179 U, 0 V moved | witnesses 4 of 4; old grid 12 false cleans, new 0 |
| Shared | ADR 0659, `40bf35ae9`, `cee52c939` | 0 (sweep runs `csr`) | `ExecutionPaths`, `--rendering hydrated`, host witnesses, `judgeSsr` |
| SC5006 | done, ADR 0660 | 0 | witnesses 5 of 5; `outside` |
| SC5003 gate | ADR 0661, `async.md` steps 1, 2 | 0 | `e02` holds; `j02 j03 j06` wait for steps 6 and 8 |
| `<Dynamic>` | ADR 0662, `async.md` steps 3, 5 | 0 | `p-dyn` holds; `m1` matrix: 0 false positives on the note's mounts; 8 patterns left the open list, 381 open |

| Merge | `059beb67a` from Track W, the first merge | 0 moved by the merge | `make verify` green, `check.mjs all` 181 of 335 (no keep, regress or warn fails), HOST 20 of 24, patterns 381 open |
| Member types | ADR 0663 | +17 U, all SC9012, 0 V moved | ViewerToolbar explained: a member's type answered for its receiver. `outside`, program in `patterns/values/sibling-type-go` |
| `clientOnly`, `lazy` + SSR | ADR 0664, `async.md` step 4 | see below | `ExecutionPaths` `DeferredMount` arms; HOST 22 of 25, 3 `NoHydration` rows left; 4 patterns left the open list, 377 open |
| Open provenance, `lazy`, method reads, cover | ADRs 0665-0668, `async.md` steps 6-8, fuzz contexts | -4 SC1001 V, +687 U (434 SC5003, 128 SC5001, 125 SC9012), `sweep-6.json`: 17,312 U, 379 V | SC5003 5 of 5; 8 patterns left, 369 open; 547 of the U are inferay, whose `@contracts` points at a missing build, so its types are `any`. The 4 V are `useSearch` in `probus-hk`, tell the owner |
| SC1002 + destructure windows | ADRs 0669, 0670 | 0 V moved, +2 SC1003 U, `sweep-7.json` | proved: a named `.then` handler; now uncertifiable: a parameter after an await. Witnesses 3 of 4, `c05` waits for the census. `then-named` 513 old, 20 new, held |

| SC8014, SC8015 | ADR 0671 | +245 U (40 SC8014, 205 SC8015), 0 V moved, `sweep-8014.json`: 17559 U, 379 V | proved: a store receiver; now uncertifiable: a test or receiver that may read, `any`, a mixed union. Witnesses 7 of 7. `d04` carries `proved: true`, `check.mjs` asks for a violation there. `outside` |
| Merge 2 | `2f06d0ab8` from Track W, 8 commits | coverage moved nothing | `make verify` green, `check.mjs all` 206 of 336 (127 clean, 3 host `NoHydration`, no keep, regress or warn fails), patterns 368 open. Stale pins fixed: tsc-oracle and obligation cases on `0` (W's ADR 0707: a numeric literal union stays open), WASM parity `dotdot-fixture` and `w3` (R's ADR 0665) |
| SC3001 blocker | ADR 0672 | 0 moved, 2 sites gain the blocker, `sweep-3001.json`: 17,853 U, 377 V (the 2 V and +292 U are W's, ADRs 0708-0711, measured across merge 2) | W's handover. SC3001 13 of 13 |
| SC1001 steps 2, 3 | ADR 0673 | +785 U (765 SC1001, 13 SC5001, 6 SC1004, 1 SC9012), 0 V moved, `sweep-1001a.json`: 18,638 U, 377 V | now uncertifiable: a read only a hidden call runs. Witnesses SC1001 18 of 18. `mx-recall` whole set: 0 outcome moves against `r3001`, same 152 false cleans. Patterns `hidden-helper-*`. Left: steps 0, 4-9, D1, D3, an arm-only helper, ADR 0664's limit |

| Merge 3 | `9af0edf84`, W to `9de5c2bd3`, plus the decisions budget | coverage: 3 W SC4002 rows | `verify.sh` green only with `typefacts-test` moved after the lanes (scratch `verify-seq.sh`): the memory test's 8 MiB arena bound fails 3 of 3 inside the parallel lanes, passes 6 of 6 alone, producer unchanged. Limit not touched: tell the owner. `check.mjs all --chrome` 218 of 336, 115 clean, 3 host, 0 `CHROME` rows: the harness fix changed no witness |
| Bound, cross-file | ADR 0674, `c245a4ad5` | -132 U against `sweep-1001a` (includes W's 70), 0 V moved, `sweep-f2.json`: 18,506 U, 377 V | proved: `[1].map(h)` in a body; now uncertifiable: stops in the walk, hand-offs from hidden members and other files; proved silent: 214 sites held only by a type query. `outside`, 13 patterns. Baseline for the next rule: `sweep-f2.json`, build `rf2/` |

| SC1001 steps 0, 4, 6, 7 | `e918518de`, ADRs 0675, 0676 | -4 U, -1 V (`CloudStorageContext:238`, V to U, no Chrome truth: the file does not load on rc.21), `sweep-s4.json`: 18,502 U, 376 V | proved V: `r11 r18 r19 z01 n14`, other store, signal's own. Proved silent: `n06 r15 z03 z05 z08`, tracked effect, after every await. Now uncertifiable: `r13 r14`, `storePath`. Step 5 skipped: `n13 n22 n23` hold. Step 8: done by step 0's hint. Left: D1, D3, D6, D15, ADR 0664's limit, arm-only helper, Q4/Q5/C-D4 (step 9), `r-dyn` check. `outside`: a tuple destructure is not in the model. Patterns 319 open (44 FP, 272 FC, 3 no run). Note `z02 r12` stay U: SC2001 and the read both say a handler's setter call is unplaced, which is W's write role |

| Merge 4 | `5da64f0a3`, W to `dbc66107a` (SC4001) | coverage 0 moved | `rediscovery.json` by union; `check.mjs all` 218 of 336 |
| 12 inconclusive | `b3130c6af`, `undriven`; then `evt-*-fire-modal` closes its modal | n/a | All 12 at `a92553232`: a `dny-*` program's own timer disables or removes the driven button, Playwright's click waits out 12 s. 3 of 3 reruns, so no race, and those contexts had no oracle at all. Now 725 of 725 identical, 0 inconclusive |
| D3, arm-only helper, 0664's limit, D6, D15 | `32fb19025`, ADRs 0677, 0678 | 0 moved, `sweep-d6.json` | proved silent: a helper or tag only `isServer` reaches, `render(Local, el)`; proved V: a pending read there; now uncertifiable: a deferred helper, `<NoHydration>` with a hydrating pass, a server-only exported function. HOST 25 of 25 |
| C-D2 | `c84106c8b`, ADR 0679 | 0 moved, `sweep-cd2.json` | proved silent: a function only handed to a scheduler, or only called from handlers; now uncertifiable: object host, returned closure, helper argument. 13 false positives left, 306 open |
| Blockers | ADR 0680, `scripts/flip/blockers.mjs` | 0 moved, `sweep-bl.json`, 18,502 U, 376 V | 9,216 rows had no blocker. Rows are `blocker:<old context>`; `contextKind` reads a row-derived head as it read a function name. `check.mjs` fails an unnamed row. A new escalation needs a blocker, and a row-derived one goes in `ROW_BLOCKERS` |

| Step 9 | `8a2de6a8e`, ADRs 0681, 0682; subclass ADR 0683 | ADR 0683: +68 U `app-game` (67 SC9012, 1 SC2003), 0 V, `sweep-sub.json`, 18,570 U, 376 V | `T-mount-from-cleanup` now uncertifiable `refused-creation-unrun`; `e511` uncertifiable `callee-callback-timing`; `g04` uncertifiable `function-run-unplaced` + `invocation-context-unproven`; `d05 d06 g24` uncertifiable, `d02 d04` close too. Patterns 298 open (25 FP, 270 FC, 3 no run) |

| H-D5 | `e9390bbde`, ADR 0684 | 0 moved | now uncertifiable: a component handed on by its own reference. `w6 x1 x2 x3 x5` |
| H-D4 | `0a2df93f6`, ADR 0685 | 0 moved | now uncertifiable: `valueOf`, `__defineGetter__`, `__defineSetter__` on a closed object. 3 FP, 2 FC left |
| F0 steps 1-3 | ADR 0686, `interproc/completeness.rs` | +125 SC9012 U, 0 V moved, 0 removed, `sweep-f0.json`, 18,695 U, 376 V | 22 false cleans left, 266 open (22 FP, 241 FC, 3 no run). `v42` passes, 222 of 336. Left of F0: `handed_value`'s call arms (`v08 f01 f12`), `ArrayMembers` (`f02`), ADR 0509's text |

| Merge 5 | W to `dfaf076df`, ADRs 0687, 0688 | 0 moved, `sweep-m5.json`, 18,695 U, 376 V, 0 unnamed | SC4001 `owner-varies-by-caller` by field and wording; SC2002 and SC3001 already named (ADRs 0680, 0672): W's note is stale; `sort.call(store)` is `function-call-target-unresolved`. 248 open (6 FP, 239 FC, 3 no run) |

Grid at `a92553232`, `mx-recall` whole set, STRICT_READ_UNTRACKED: 62,480 programs, 0 false positive, 152 false cleans, the two known `ref-*` groups; 12 oracle runs inconclusive, not read.

Handoff at `66684ac14` (merge 5, `make verify` green through scratch `verify-seq.sh`). Left of the owner's order, in order: F0 steps 4-6 (`handed_value`'s call arms `v08 f01 f12`, `ArrayMembers` `f02`, ADR 0509's text), F-C, F-D, step 5's census with `value_use.rs`, F-B, step 6's bounds audit (not started). Next ADR number 0689, R's range ends at 0699. A `new Map()` receiver could be proved plain as an array literal is (ADR 0683's +68 U). Review wanted: ADR 0683 +68 U, ADR 0686 +125 U.

Next agent: F0 steps 4-6, F-C, F-D, the census, the bounds audit. Baseline sweep `sweep-m5.json`, build `rm5/`. `make coverage` prints only the first moved row of a fixture: compare snapshots row by row before an update. Scratch `gates.sh <tag>` builds and runs coverage; `verify-seq.sh` is `make verify` with `typefacts-test` after the lanes. Build `rs4/`, sweep `sweep-s4.json`.

Correction: `e918518de`'s message says 24 entries and 343; the run shows 25 left, 342 open. `a92553232`'s says 24 false positives left the list; it was 23 (67 to 44, 342 to 319 open).

The 152 were not one cause, and the "ref role" label was inferred. Measured: 72 `ref-row`, `callee_roles.control-flow`; 80 layered `ref-named`/`ref-factory`, `callee_roles.named-callback-untracked` (logged on `ref-named` only). Two `open-defects.json` entries now. Not fixed.

A generator context lands inside the component, where a helper has a row, so `hidden-*` contexts passed on the old build and were dropped: check the old build fails before trusting a shape. Use scratch `chk.sh`, not `research/chk.sh`: its oracle path dangles, `solid-js` does not resolve, and everything reads as certified. Check `exitErrors` in a sweep: a project killed at 900 s reads as hundreds of removed findings.

Next, owner's order: (1) done, ADR 0659, `ExecutionPaths`, `--rendering hydrated`; (2) done, `scripts/flip/host-witnesses.json`, `check.mjs HOST` 20 of 24, the other 4 marked `open`; `judgeSsr` reads every client rule; (3) SC5003+SC5001 by `async.md`, steps 0-5 done, next 6, 7, 8 and the fuzz contexts (`dyn-jsx-*`, `portal-*`, `*-later`, `BODY_LABEL`, `<Portal>` in a kept grid); (4) SC1002, SC8014, SC8015, SC1001 (add the arm fact and a `NoHydration` blocker: `unplaced_run` misses D1, D3, because both are `MayRun`), SC9012. Latest sweep: `sweep-co.json`, 16,652 U, 383 V (the merge added 25, W's ADR 0705; ADR 0663 added 17).

`sc1001.md`, `f0.md`, `sc9012-census.md` beat the plan: reread the row's section before each rule. They add to rules in between: SC5003+SC5001 owns `dynamic_component_entry`, the `<Dynamic>` hint, check `cases3-p03` there, and maybe C-D4 (`n01 n03 n06`); SC1001 has 10 steps and needs `patterns.mjs:130`; H-D5 lands before the census; `check.mjs` needs SC9012 for 9 ids (W's 8 and `o12`) in the census commit.

Open for the census: `const hook = c ? useThing : useThing; const t = hook(); createMemo(() => { t.go(1); ... })` is certified and Chrome throws (`research/probes/v1/a`, `v2/a-nosib`). A hook picked by a conditional reaches no `handed_value` arm. Not a sibling-type case. Close it with F0 or the census, and add it to the fuzzer there.

Owner's correction, 2026-11-05: **a moved true violation stops the track, unless proved locally.** The `o02` trade is not to be applied again. The 4 `useSearch` sites stay recorded for the owner: the memo returns a bare type parameter from a caller's `select`, and ADRs 0266 and 0300 close no return.

Next: SC8014, SC8015, then SC1001, H-D5, F0, SC9012. Latest sweep `sweep-7.json`, build `r7/`. Helpers: `bld.sh <tag>`, `f.sh <tag> [dir]` (one fixture), `tab.py`, `diffg.py <old> <new>` (grid outcome moves), `snapd.py` (moved snapshot rows). Grid false cleans, final numbers, each held in `open-defects.json` with a cause: 690 ASYNC_OUTSIDE (census), 71 pending `async-value-member` (member-tag getter), 152 `store-dstr-*` in `ref-row`, `ref-named`, `ref-factory` (ref role), 20 `then-named` (census). Diffed by id against `rco`: none is new. A parameter compute (`createMemo(options)`) is open: a fact for a value's signature would close it.

Handoff at ADR 0664: helpers `chk.sh` (scratch, oracle path fixed), `rd.py <cmp> <sweep> x` (each added site with its source line), `rm/` (merge build), `r9012/`, `rco/`. Do first: step 6 (`computation_provenance`, four discovery sites, wording flag, C-D4), then 7 (`lazy` tag row), 8 (`j06`), the fuzz contexts. Step 6 reaches W: an `any`-computed `createOptimistic` starts to hold, tell W in the commit.

Surprises:
- Gate and commit with `&&` or `if`: `;` committed a red verify-delta once (clippy), amended before pushing. An item after a `#[cfg(test)] mod` fails clippy.
- `rm -rf *` after a `cd` is refused by the harness: use a fresh probe directory. zsh does not word-split `$ids`: `redisc.py` then gets one id. `patterns.mjs` takes `--checker`, not `--checker-bin`, and silently uses the default build with the wrong flag.
- Read a grid's whole outcome line, and diff by id against the previous build, before committing: 596 false cleans sat unlisted past ADR 0668 because the run printed them under "more". A background `timeout` is the background limit: pass a long one.
- A recall-only code's own cells go in `recallContexts`, with `contexts: []`, or `recall.test.mjs` counts them as base cells.
- `MISUSE_CODES` left out advisory codes, so SC5003 never had a grid until ADR 0668.
- ADR 0659 limits to carry: a site in a helper only an arm calls keeps its finding; dependency source is the client graph on both paths; the sweep still runs `csr`.
- Chain gate and commit with `&&` or an explicit test: a pipeline hid a failed `ownership-gate` once, and the commit had to be amended before pushing.
- `ef46d0ac6` and `aaaf31cad`: verify-delta rows for rule pages, `.gitignore` and `scripts/flip/`. A progress-file edit now stays in the fast plan.
- `ef46d0ac6` (Track W too): verify-delta maps `docs/rules/` and `.gitignore`. Its message says 27 of 27 tests; it was 26 of 26. Pushed, so not amended.
- `type_descriptor_spans` also turns on callability, and a descriptor is evidence other consumers read: demanding one on a coerced operand moved an unrelated SC9012. Use `callability_only_spans` when only callability is wanted.
- `ir-read-lint` matches by name: `.functions.iter()` in a file that declares a `functions` map counts as a hash iteration.
- `diagnostics_process.rs` pins per-fixture finding counts: extending a fixture moves them. `make coverage` does not show this; `nt.sh` does.
- `make verify` selects `repro-causes`, `verify-delta` does not for a rule change. Run it when a rule moves or rewords a finding: `fixtures/uncertifiable-repro/cases.json` pins causes, and 5 had gone stale before the first merge. W's messages too: `uncertifiable-causes.mjs` matches wording.
- A generator context can only wrap `stmts`: a member that must itself be the setter is outside it. Keep such a program in `scripts/fuzz/patterns/`, where the old build fails it as `NEW false-clean`.
- A JSX element in a context's `body` is created, not rendered. `children(() => <X />)()` forces it.
- `ce6a831b2` and `ae2327986` are base repairs for Track W: 3 process tests, the read ratchet, WASM parity. All left behind by deliberate changes; `make verify` had never run on the base.
- `cases.json` and `rediscovery.json` are append-only: edit by text. `json.dumps` reformats the whole file and makes a certain merge conflict worse.
- The branch cut had 3 red Rust tests (none ran `make verify` there); fixed in the commit `test(flip-base): ...`, for Track W to pick up. Rust tests are 1,252 of 1,252 from then on; `nt.sh` prints anything new. Pass `BASE=<last rule commit>` to verify-delta, or it maps thousands of paths against `origin/main`.
- `scripts/fuzz/install` needs `npm ci` too (Chrome host); the plan names only the oracle install.
- `view` children in a generator context must be numbers: build a boundary in `body` with `domExpr`.

Stop report, 2026-11-06, the 72 of `pos-flush-fn` (STORE_WRITE_OUTSIDE_SETTER, `nested-assign`, `nested-push`, mount `dynamic`): whole `mx-recall` on `rm5` showed 60 wrong-rule and 12 false positives. Chrome reports only the store write, so SC1001's no-window claim holds. Same checker, older judges (`scratch judge.sh`, partial runs: the oracle stops on the pinned rc): `7e9b9d5ee^` 0, `7e9b9d5ee` (ADR 0641, `beside`) 72 wrong-rule, `5b8a1d7b5^` 72, `5b8a1d7b5` 60 + 12. Every saved build back to `base` gives the same 72 under today's judge. So the judge brought them back, not the checker. The 12 were a line with no counter (`wrapUse`): the generator now counts it. A first fix in `silenceMeasured` was withdrawn on the owner's word and must not return.
