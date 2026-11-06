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

## Handoff at 976e73028, 2026-11-06

Pushed. Tree clean. Fresh Opus 5.5 high agent: read this section first. Census research runs separately and writes `/tmp/sc-scratch.noindex/census-research/report.md`: read it before step 4.

| Merge 6 | W to `91311857e`, ADRs 0750, 0751 | +1 U (SC6002 `search-depth-limit`, inferay `useQueryResource:140`), 0 V moved, `sweep-t3.json`, 21,617 U, 377 V, 0 unnamed | The first commit of this merge did not compile. Repair found two real defects: W's rows joined blocker and detail with a space, so ADR 0680 called them unnamed; and SC2006, SC2008, SC6002 had lost ADRs 0673, 0674's arms, which dropped 3 SC6002 U rows from `coverage`. `unplaced_blocker` now asks both walks. `check.mjs all` 296 of 336, 40 fail, all kind `clean` (32 SC9012, 4 SC2003, 2 SC2009, 1 SC1002, 1 SC1003), no keep, regress or warn. W's six named witnesses are all that fail of its list: c04 and j05 pass |

| Merge 7 | `cd131fc23`, W's final head, ADR 0752 | +258 SC2009 U, all compass-ui, 257 in test files, 0 V moved, `sweep-t4.json`, 21,875 U, 377 V, 0 unnamed | `make verify` green, `check.mjs all` 296 of 336, same 40, HOST 25 of 25, patterns 139 open. W's note calls the project inferay `ui`; this machine's manifest names it compass-ui. `value_reaches` is R's, W says why in `bounds-audit.md` |
| Step 2 | case 32, `verify-delta` | 0 moved, no analysis change | Case 32's `props.n = 1` removal dropped a negative control: restored as two cases, exported U, closed twin silent, 141 cases hold. `FINDING_GATES`: every path that can move a finding selects coverage, ownership, tsc-oracle and obligation-audit; solid-facts and the dialect rows selected none of them before |
| SC1001 `flush(fn)` | ADR 0694, the owner's ruling | 0 moved in the corpus; `pos-flush-fn`: 72 of 2,514 programs, 84 SC1001 rows V to U, 0 outcomes | now uncertifiable: a store Get that only leads to a write, `write-path-read-unproven`. Kept V: a real read, `+=`, an alias, a project `push`. `expected-silent.json` does not fit: the judge excuses the no-window hint, so no old build fails, and the judge stayed untouched |
| Bounds and P2 | ADR 0695 | +329 U (289 sites), -2 U, 0 V moved, `sweep-b10.json`, 22,202 U, 377 V | now uncertifiable: a read in a cleanup with a disposer in the project, a called handle, a function no primitive placed, a forwarded callback, a read past 32 dynamic climbs. Also now uncertifiable: a chain past 3 helpers after an await (70 false cleans, found by `chain-N`, measured by id, 0 proved). Proved silent kept: `untrack(fn)`, `onSettled(fn)`, an action's step. Also proved silent: a closure only a host keeps with no sync dispatch in the project, and a compute's returned closure an apply only `void`s. The draft as written lost 3 true V and gave +642 U: narrowed, not landed that way. `value_reaches`, `may_run_in_leaf_owner_within`: `local_access.rs`, R1's |
| Round 1 | part 8 integrates: R2 to `c6677ddac` plus item 7, R3 to `63c15d6df`, memory review items 2, 3, 5, 6, 8, `focus.mjs` cap. R1's `8428ec497` dropped: `clean-interval-hook` | 24,515 to 25,343 U, 377 V, 0 V moved, 0 unnamed, `sweep-m5.json` | `check.mjs all` 299 of 336, 37 kind clean, HOST 25 of 25, patterns 92 open (2 FP, 90 FC). The runaway was `composite_callee_walk` on `i++`. F1 and its two memos stay on `claude/flip-r2-wip` |
**Baselines.** Build `rk2/`, sweep `sweep-k2.json`: 21,616 U, 377 V, 0 unnamed, 0 exit errors. Patterns 139 open (3 FP, 133 FC, 3 no run). `make verify` green on this tree through verify-delta's escalation, in the lanes as written. Set `SOLID_CHECKER_TSRS_BIN=/Users/thomas/Documents/Github/tsrs/target/release/tsrs` for old-revision judges (scratch `judge.sh <rev> <bindir>`).

**Merge first.** `origin/claude/flip-writes` moved 2 commits past the last merge: `138ff86e7` (ADR 0750, a walk past its depth limit is undecided) and `91311857e` (ADR 0751, a value chain past its limit). They overlap step 6 below. W took 0750 and 0751, so continue from 0752 if R's range runs out.

**Next ADR.** Used 0689, 0690, 0691, 0693, 0696. 0692 (census) and 0695 (bounds) have text drafted in scratch `w0/all.diff`. 0694, 0697-0699 free.

**Done of the owner's order.**
1. Grids on the merged head: `mx-recall` 607,719 programs, 862 FC (690 ASYNC_OUTSIDE, 152 store destructures, 20 `then-named`), 196 wrong-rule, 12 FP. `lg-recall` 372,353 (lean, `rm5`, restored judge), 0 FP, 8 disturbed, all the unplaceable LOWERCASE_EVENT_ATTRIBUTE. Both before `976e73028`.
2. F0 rest, F-C, F-D: done, ADRs 0689, 0690. Proved V: `children`, `mapArray`, `repeat`, `createMemo(fn)()`. Now uncertifiable: handed call results, `.toArray()`.
3. C-D4: already held. `n01 n03 n06` give SC1001 U and SC5001 U, with no code change this session.
5. `new Map()`: done, proved, ADR 0693.
Owner's rulings: done, ADR 0696 (`c16`, `h17`, unevaluable branch). `scripts/fuzz/expected-silent.json` holds the ruled programs.
The 152: all closed, now uncertifiable, ADR 0691. `ref-factory` measured 40, so the inference held.

**Left, in order, all drafted in scratch `/tmp/sc-scratch.noindex/flip-reads/w0/`, none landed.**
4. Census, ADR 0692: `hold/value_use.rs`, `census-block.rs` (goes back after the `open_members` block in `reactive_analysis.rs`), `p-census.diff`, `gen-values.py`, `hold/fixtures/value-use-census`. Add `mod value_use;`. Its blockers are already in `blockers.rs`. Debug-build probes before the merge: of 239 open FC patterns, 34 stayed certified with census and bounds together; all research probes closed but `u11`, W's. Then remeasure 690 ASYNC_OUTSIDE, 71 `async-value-member`, 20 `then-named`. Open lead: `h.a()` where a hook returns `{ a: createMemo(...) }`, read in tracked JSX, is certified (`hookm`): the source has no binding, so no async row.
6. Bounds, ADR 0695: `p-bounds.diff`, `gen-chain.py`, `hold/fixtures/walk-bounds`, `notes/bounds-audit.md`. Includes the silent default `allowed-unplaced-nested`, certified where Chrome warns, and forwarded callbacks. Roles test pin already in the patch. Measure against W's ADRs 0750, 0751. Warning: a depth-limit answer of true in `value_reaches` moved 13 true V and raised 710 SC2009; the exact walk moved none. Keep the exact walk.
7. `outside` entries: not done. Known stale reasons: "ASYNC_OUTSIDE ... not MISUSE_CODES" (cells since ADR 0668), "`<Dynamic>` is not a mount" (contexts `dyn-jsx-*` exist). `sort.call` was never tried: use a text variant, kind `store-array`, `Array.prototype.sort.call(${o})`. Text variants (`FLIP_READ_VARIANTS`, `text`, `imports`, `legal`) can write most sites `outside` says a context cannot; `variantContexts` must name legal contexts too, or no legal program is generated.
Patches were cut before the merges: apply with `git apply --3way`, and `git add` new fixtures before `make coverage`, which refuses untracked stubs. Add the `.gitignore` exceptions.

**Open for the owner.**
- `flush(fn)`: SC1001 with the no-window hint on a store Get inside `flush(fn)`, in a body only `dynamic()` mounts. Do not change until a ruling is relayed. My answer in the final report: keep the warning.
- 136 `LOADING_ON_OUTSIDE_HOLD` wrong-rule: SC5003 V beside it. The oracle records ASYNC_OUTSIDE_LOADING_BOUNDARY on all 136, so the claim is true. The code is placeless, so `corroborated` cannot place it, and SC5006 is not reported for these rows. Not on `open-defects.json`, not changed.
- The 72 `pos-flush-fn`: closed in the judge and the generator (`922e1e7d1`). Cause and bisect are in that commit.
- ADR 0683 +68, ADR 0686 +125, and `976e73028` +1,315: over the review threshold of 20, unreviewed.

**W's leftovers, not started:** nested identity, `Object.entries`, destructured loop binding, and the SC2001 and SC4002 wording (`lib.rs`, the owner's "add wording for W's new blockers"). `u11` stays W's.

**Traps from this session.** Grid processes read `scripts/fuzz/*.mjs` live: no edit there while a grid runs. A `silenceMeasured` fallback was withdrawn on the owner's word: a finding's line needs its own counter. verify-delta skips `tsc-oracle` unless a path escalates: `8833e6bfa` went out with that gate red. `make coverage` shows one row per fixture: compare snapshots row by row. The commit message of `976e73028` rests on grids from before its last two edits; rerun the whole set after the census.

## R2 (agent), from 2026-11-06

Base `879eca007` reports, branch `claude/flip-r2`, ADRs 0780-0789. Scratch `/tmp/sc-scratch.noindex/flip-r2`: `gate-times.tsv`, helpers `bld.sh`, `sw.sh`, `one.sh`, `ids.mjs` (id manifests for a subset of mx-recall), `defs.py`.

| Family | Proved | Now uncertifiable | Rows |
|---|---|---|---|
| List: 9 programs, 3 no-run | `a11`'s twin confirmed (STRICT_READ_UNTRACKED); `h03` is NO_OWNER_EFFECT, SC4001 stands; `e807` runs | | `e911` (`on:click` is TS2322) and `f08` (the plugin leaves `@dec` in, no way to run it; SC1001 U at its site) deleted |
| P6, P5, P10 (ADR 0780) | `b05 b06 d7-export-star-as -store`, and `Api.flush()` SC2008, which the new binding found | `h18`, `onLoad` in a control-flow callback | 139 to 130 open |
| P3, P4, F3 (ADR 0781) | | lying cast and patched global: `d01-cast d07 x30 e107 e108`, `t3-*-cast`, F3 no longer a false positive | 3 new held rows, `p3-cast-*`: an assertion away from the receiver, certified, Chrome warns. Needs the owner |
| F1, `e703` (ADR 0782) | `s1`, `s2`, `e703`, `g12`: write legal, the only mount is in a cleanup; `b-handed`, `d-mixed`, `s4` stay V | | |
## R3

Branch `claude/flip-r3`, pushed to `origin/claude/flip-r3` (since 2026-11-06 the landing rule: never push flip-reads; part 8 merges hourly). Scratch `~/EDF/tmp/flipr3` (`gate-times.tsv`, `bins/`, `s1/`...). Base build `bins/r3base` (80db411e5).

| Step | State | Moved | Notes |
|---|---|---|---|
| 1, 136 `LOADING_ON_OUTSIDE_HOLD` | judge only, no Rust | `mx-recall` LOADING_ON_OUTSIDE_HOLD, `r3base`: before 136 wrong-rule + 422 recall-gap, after 558 recall-gap; the 136 all carry `detail.holeCorroborated` (128 declined by the claiming rule, 8 by SC9012) | Artifact removed where it arises: `classify.mjs runtimeEvents` dropped the diagnostic's `loc`. `matrix.mjs` accepts a placeless code whose `loc` line opens a node enclosing the site and no other use site. `loc` is not in `positions` |

## R2b, from 2026-11-06

Branch `claude/flip-r2`, worktree `t3-7c67e91b`, ADRs 0820-0829. Baseline on the merged tree: build `rm1`, `sweep-m1.json`.

| Family | Proved | Now uncertifiable | Rows |
|---|---|---|---|
| P7 (ADRs 0821, 0822) | `export const` copy of a signal or store, read or written elsewhere; `d8-pkg-types-lie-plain` | a package's Get the walk does not close, `dependency-value-unfollowed`; a global a package installs, `dependency-global-read` | export-copy 108 FC to 108 V, legal 72 clean. Lane: 15 d8 closed, 3 `d8-pkg-plain-*` stay open. Corpus +361 U, 0 V |
| P9, `f02` (ADR 0820) | read after an `await` in an async generator a `for await` in the compute steps: `f02`, `f1`, `f6`, `f8`, `f13`, `f16` | past a `yield`, a wrapper, `next()`, `yield*`, an inline generator, a sync generator across an await, blocker `generator-stepped` | 1,488 ids: old 1,092 FC, new 0 FC, 0 FP, 200 V, 1,288 U. Legal 0 FP of 1,316. Corpus 0 moved. Patterns 89 open |

## R2c, from 2026-11-06

Same branch and worktree, scratch `$S` as R2. Baseline: build `rp9e`, sweep `sweep-p9e.json` (= `sweep-m1.json`, 0 moved).

| Step | Proved | Now uncertifiable | Rows |
|---|---|---|---|
| P7 landed (ADRs 0821, 0822) | as R2b's row | as R2b's row; `WindowQuery` in `dependency-source-proxy-store` (was silent) | build `p7i`: +361 U, -1 U, 0 V; witnesses 300 of 336, same 36 census ids as `p9e` |
| Member tag (ADR 0823) | a same-file sealed holder's tag is a call site: `async-prop-getter`, SC1001 V (`n1 n3 n4 n5`), literal clean (`n2`, `m4`) | a leaf-owner prop read whose callers do not exclude a pending getter, `prop-pending-unproven` (`m5 m6 m8-m11`) | grids: mx-src 60 FC to 0, mx-combo 26 FC to 0, 0 FP; legal 84, 0 FP. Corpus +51 U, -10 U same-start, 0 V |
| Props destructure (ADR 0824) | | `const { v } = props` with a pending getter outside tracked JSX or in a leaf owner, `async-prop-getter`; open callers in a leaf, `prop-pending-unproven` | async-value-dstr 84 ids: 60 FC to 0, 0 FP; legal 84, 0 FP. Corpus 0 moved |
| primitive-value (ADR 0825) | the 2 rows an import confirms keep the primitive's name | 78 rows: `primitive-identity-unproven`, same sites, still U | corpus 0 moved, 78 relabelled |

### R2c handoff, 2026-11-06

Head on `claude/flip-r2`: P7 `afcee743f`, lint `f6de3eb2f`, member tag `328763a21`, props destructure `44f848399`, primitive-value (ADR 0825) the commit holding this section. ADRs used: 0821-0825; 0826-0829 left. All landed with verify-delta, witnesses (300 of 336, the 36 census ids), sweep and `make verify` green. Baseline for the next slice: build `rpv1`, sweep `sweep-pv1.json`.

**Helpers added in `$S`:** `srcids.mjs` (mx-src or combo id manifest by regex), `lgsrc.mjs` (lg-src legal ids), `snapdiff.mjs` (a fixture's full finding diff against its snapshot: coverage's report caps its lists), `moved.mjs` (sweep rows grouped by blocker).

**Open, not done:**
- Imported compound holder (`import { Ui } from "./ui"; <Ui.C />`): U `prop-pending-unproven`, not proved; needs the holder file's literal through a recorded read.
- `primitive-identity-unproven`: why hoxxes-briefing, inferay, spookysoftware-www facts attribute `createOwner` to unresolved imports is not traced; W's ADR 0711 has the same root for calls.
- Lost proof: `dependency-source-window-realm` under `--runtime-realm window`, 3 V to 3 U in `f2e5f60e0` (open-work.md).
- Held for the integrator (census or `shared_reactivity.rs`): `use().v` on a returned store, `export let read = c` (open-defects P12); `d8-pkg-plain-*` (a store handed to a package function).
- Smoke: `smoke/mx-src.json` gained 60 `async-value-dstr` ids by hand; reselect from a full run at the final stage.

**Traps found:** `push_defect` keeps one SC9012 per start byte, so a new obligation hides another at the same start (sites stay U). For a shorthand `{ C }` the value's symbol is the property's: use `reference_declaration`. `run.mjs` copies only `App.tsx`. A coverage fixture's stubs must be staged (`git add`) before coverage counts them as tracked.

## R2d, from 2026-11-07

Same branch, worktree and scratch `$S` as R2c. Baseline: build `rpv1`, sweep `sweep-pv1.json`.

| Step | Proved | Now uncertifiable | Rows |
|---|---|---|---|
| primitive identity (ADR 0826) | root cause: `symbol_names` named the unknown symbol (root of every unresolved import) after `@solidjs/router`'s unresolved `import { createOwner } from "solid-js"`; a root with no declaration is named by nothing | | corpus `pi1` vs `pv1`: 0 V moved, -82 U (SC9012 78, SC3001 2, SC6001 2), +8 SC9012 U once hidden by one row per start. Witnesses 300 of 336, same fails. Coverage 0 moved |
| unresolved import (ADR 0827), the coordinator's challenge to 0826 | | every runtime use of an import the compiler did not resolve, `import-unresolved` (was certified: JSX call, body read, effect argument, tag, write) | corpus `iu1` vs `pi1`: +7,109 U, 0 removed, 0 V moved. Coverage: 15 fixtures, additions only, read |

## R2 handoff, 2026-11-06

For a fresh agent. Read `scripts/flip/notes/agents-brief.md`, then the rules below, which replace the brief where they differ. Branch `claude/flip-r2`, worktree `t3-7c67e91b`, no merge or push unless told. Scratch `$S` is `/Users/thomas/dev/sc-scratch.noindex/flip-r2`; never `/tmp`, a reboot wipes it.

**Done and gated:** the list, P5, P6, P10 (ADR 0780); P3, P4, F3 (ADR 0781); F1 (ADR 0782); memory review items 4 and 7 and the handle fan-out, memoized by span and depth. ADR numbers left: 0783-0789. Patterns 120 open, 0 false positives.

**Left, in order:** P9 and `f02`; P7; member tag; `primitive-value`. Plans: `research-patterns.md`, `research-sc9012.md` section 3.

**Check first.** `$S/safe.sh` refuses a tree with `8428ec497` and without `10da57271`. Merge `origin/claude/flip-reads` only after part 8 reports round 1, then one build, no full gate run. If part 8 says this branch broke the merged tree, that comes first.

**Machine rules, owner's, current.** Every run that starts `solid-checker-rust`: `$S/g.sh <command>` (load under 30, `sc-heavy`, `sc-guard.sh 12`; report if the guard fires). One heavy check at a time. Sweeps `--jobs 3`. Release builds bare with `CARGO_BUILD_JOBS=6`. No `focus.mjs` or `make fuzz-focus` until part 8 caps it: ids only.

**Helpers in `$S`.** `bld.sh <tag>` makes `r<tag>`. `sw.sh <tag> <prev>`, `lane.sh <tag> [only,only]`, `cov.sh <tag> [update]`, `vd.sh <tag> <base>`, `lib.sh <tag>`, `chk.sh <cases> <bindir>` (wrap in `g.sh`), `probe.sh` (tsc, Chrome, checker), `one.sh` and `lg.sh` (mx-recall and lg-recall subsets), `ids.mjs` (id manifest by regex), `defs.py rm|add` (open-defects and rediscovery by text), `union.py` was lost in the reboot: rewrite it if a merge conflicts. Baselines: build `rca`, sweep `sweep-ca.json`, old-build `rbase` (c6677ddac).

**P9 and `f02`, from reading, unbuilt.** `after_await.rs:287` skips an async generator as an entry and `:1506` returns on a generator helper, and a `for await` operand sits before its own suspension, so no region reaches it. Plan: a pass beside `unlisted_async_computations`: for each tracked compute, each call in it whose `function_called_at` is an async generator, every accessor read after the generator's first await or yield is a `ReactiveReadAfterAwait` with `GeneratorStepped`. `file.ast.yields` for the yield boundary. Stays quiet: `q4`, a read before the first suspension. Check `q2`, already U, for a duplicate row.

**P7, from reading, unbuilt.** The report's `resolver_dependencies.rs` is not at that path: find it with `rg --files rust`. Programs `values/d8-pkg-getter-export`, `-side-effect-import`, `-types-lie-plain`.

**Traps.**
- A report's line is a lead: F1's named site is never reached. Use the role log, `SOLID_CHECKER_IR_ROLE_LOG=<file>`.
- `functions_owning` is not innermost first.
- ADR 0367: no decision reads type text, `descriptor_text_gate` fails it. Ask the producer for a fact.
- `ir-read-lint` counts raw cross-file reads: route one through an existing helper.
- The fuzz model refuses `call(member(dom))` and `call(dom)`: hand the callback by name with `domExpr`.
- A local memo needs depth in its key if the bound changes the answer, and recorded reads as in `returned_invocation_sites_memo`.

**For the owner, leave as they are:** `types/p3-cast-*` (an assertion away from the receiver, held in open-defects), and the unknown-key global trade in `docs/open-work.md`.

| 2, SC2003 silent tail | `bfd336ce8`, ADR 0790; `0ccd0aa2e` (3 patterns) | +372 / -2 SC2003 U, -1 SC9012 U, 0 V moved | 44 Chrome-warning probes: base silent on 36, now 5 proved V, 39 U. False positive fixed: a write root took its first token's symbol. Demand for argumentless write-root calls. Grid: 168 FC + 48 FP on r3base, 0 on the fix; patterns v09, v10, d6-comma closed |
| smoke refresh | `3b4e446bc` | n/a | reselected from a full VM run at bfd336ce8, 406 leaving programs named; V-D1 removed, attributed to 061f71bd2 (ADR 0718) by id; 5 entries added, open before R3 |
| 3, P8 | `ff0e75ed6`, ADR 0791, narrowed | 0 | U: a delete behind an optional chain (p9, p10). Not closed: p1-p8 (computed key, index, required `T \| undefined`, `unknown`), owed a producer answer, below |

**Method for old/new fuzz checks (owner, 2026-11-06): id sets, in the VM.** Never a whole grid while fixing. (1) Build the id set: the step's new fuzz shapes, the rows that moved, the rediscovery ids. From a finished run, `scripts/fuzz/rerun-ids.mjs <run dir> --out <dir> --outcomes … --grids …` writes `{ generator, seed, ids }` manifests; a manifest can also be written by hand, and `legal.mjs` refuses an id it does not generate, so build legal ids from `legalCells` + `recallLegalShapes` + `legalId` (scratch `tools/lg-ids.mjs`). `matrix.mjs`/`legal.mjs --grid --recall --seed 1 --codes C --contexts … --ids <manifest> --confirm-with-chrome --checker-bin <bin>` generate only those. (2) Run both builds in the `sc` Colima VM: `FUZZ_VM_PROFILE=sc scripts/fuzz/vm.sh sync` (new build: the clone at this checkout), the old build as a worktree of its commit under `/mnt/lima-colima/sc/<tag>` with its own `CARGO_TARGET_DIR` (`cargo +1.99.1 build --release -p solid-facts-backend --bin solid-checker-rust`), then one `vm.sh detach <log> bash <script>` running every command in sequence (a detach syncs first: never a second one while a run uses the clone), `vm.sh pull <vm dir> <host dir>`, diff by id. Host paths under `/Users/…` are mounted at the same path in the VM; `~` there is the guest's home. A macOS run past 30 minutes in the background is killed (`focus-old` stopped at 20,250 of 25,914).

### R3 handoff, head `ff0e75ed6` (2026-11-06)

Pushed to `origin/claude/flip-r3`. Tree clean. Context ran out; next agent: start step 4 after a plain merge of `origin/claude/flip-reads` and a build.

**Done.** Step 1 (`4d9924344`). Step 2 (`bfd336ce8`, ADR 0790) and its follow-up (`0ccd0aa2e`). Smoke refresh (`3b4e446bc`). Step 3 narrowed (`ff0e75ed6`, ADR 0791). Merge of flip-reads at `8ff25dca5`. Commit messages carry the measurements.

**open-defects.json changes, attributed by id in the VM.**
- Removed V-D1 (lg- noop-*, false-positive): fixed by `061f71bd2` (W, ADR 0718). 061f71bd2^: 53 FP and 22 unexecuted of 75; at 061f71bd2: 0 and 0.
- Removed patterns api/v09-store-write-nullish, api/v10-store-write-let, values/d6-comma: certified FC on 80db411e5, fine on bfd336ce8 (ADR 0790).
- Added, all open on 80db411e5 too:
  - PENDING_ASYNC_FORBIDDEN_SCOPE async-value-member (member-tag getter);
  - RESOLVE / UNTIL / CAUGHT_REACTIVE_READ handed-flush (g2d named hand-off);
  - CAUGHT_REACTIVE_READ mount-named (false positive).

**P8's producer question (p1-p8 open).**
- `delete st["a"]`, `st.rec[key]`, `st.rec.k`, `st.items[0]`, `st.o["c"]`, `st.nick` (required `string | undefined`), `st.u` (`unknown`): tsc accepts each without `exactOptionalPropertyTypes`, and Chrome warns on each.
- Type Facts gives a computed operand no entity: `demand_plan.rs` `collect_delete_spans` skips it.
- A static operand's entity carries only `writableOptionalProperty` (apps/solid-typefacts `type_facts.rs` `writable_optional_property_at`): `Property | Optional` and not readonly, else `false`. It carries no symbol, no domain and no compiler options. So `delete st.b` (TS2790) and `delete st.nick` / `st.u` (accepted) both read `false`.
- An uncertifiable there is visible to the ownership gate, which then fails `delete-required-property/001` (invariant 1). Tried and reverted.
- Owed: a tri-state producer answer mirroring `checkDeleteExpression`: resolved property symbol (none for an index signature or a non-literal key), readonly, strictNullChecks, exactOptionalPropertyTypes, `any`/`unknown`/`never`. Demand computed operands as well.
- Then the consumer gives V for an accepted optional literal key, U where tsc accepts and the store is linked, and silence where tsc rejects.
- Probes: scratch `s3/cases/p1-p10, n1, n2, c1, c2, q1`.

**Left, in order.**
- Step 4 (the `outside` entries). Leads are in `~/EDF/tmp/flipr3/prep-steps-3-4.md`: read only, untested, reproduce in Chrome and tsc yourself.
  - 214, 215, 218, 219 are mostly closed by ADR 0790. They still need shapes (`sw-object-root`, `sw-generator`, `sw-assign-copy`, `sw-param-array-pattern`, `sw-foreach-pattern`, `sw-loop-head`, `sw-assign-alias`), plus one VM build of `1232c2f2c^` to turn their `outside` into ids.
  - 217's `.bind(...)()` and `Reflect.apply`: R1 added NET_STORE_CALLS shapes (`net-push-bind`, `net-push-reflect`), so check what is still certified before writing code.
  - The 13 stale reasons: drafts in the prep file.
- Step 5: SC2001 and SC4002 wording in lib.rs.

**Blocker.** 198/199 (g2d: `[1].map(hf)` in tracked JSX) need `local_access.rs` `handed_role` / `reports_untracked_read`. Not R3's file; the census handed-flush false cleans are the same cause.

**Owner items.**
- mx-census false positive `mx-caught-reactive-read-mount-named-async-direct-none`: SC5001 violation at a `try { a1(); } catch {}` under a module-level `render(f1, el)`, Chrome silent. The same on 80db411e5.
- Certified false cleans `mx-resolve-in-reactive-scope-handed-flush-x-direct-none` and `mx-until-in-reactive-scope-handed-flush-x-direct-none` (g2d cause).
- The 7 mx-census harness rows, which are the 7 inconclusive oracle runs. 6 are mount-named, where `page.waitForFunction` timed out at 12 s; 1 is flw-settled-report-error, where the browser bootstrap did not bind. `fuzz-smoke` exits 2 on them, so the patterns lane needs its own run (`patterns.mjs`).

**Method.**
- Fuzz checks run as id sets in the `sc` VM. Write manifests with scratch `tools/mx-ids.mjs` (`recallShapes` + `shapeId`), `tools/lg-ids.mjs`, or `tools/smoke-ids.mjs` (a smoke run's flagged ids, with each grid's own args).
- Old build: the VM worktree `/mnt/lima-colima/sc/r3base`, binary `/mnt/lima-colima/sc/r3base-target/release/solid-checker-rust` (80db411e5). Worktrees `wt-061f71bd2[p]` and `tgt-061f71bd2[p]` are there too.
- New build: the clone after `vm.sh sync` (`rust/target/release`).
- One `vm.sh detach` per batch. Clear your own `ram/scratch/focus-*` and `ram/tmp/*` when the tmpfs fills: a smoke run refuses under about 5 GiB free.
- `patterns.mjs --only` takes one substring before the merge at `8ff25dca5`, and a comma list after it.
- Corpus sweeps on macOS: about a minute each. Baselines: `sweep-r3base.json` (21,674 U, 377 V), `sweep-m3.json` (merge, 24,659 U, 377 V).
- Probes: scratch `tools/probe.mjs <dir> --bin <checker> [--chrome]` (tsc + checker + Chrome) and `tools/one.sh` (one case).

### R3b, steps 4 and 5 (2026-11-06)

Branch `claude/flip-r3`, pushed. Commits `a98bc8e58` (step 4, no checker change) and `f1ed5195d` (step 5, ADR 0792, wording only). Scratch `~/EDF/tmp/flipr3/s4` (`cases/` hand probes, `ids/` manifests, `vm*.sh`, `tab.mjs <dir> <tag...>` tabulates programs.jsonl across builds, `tools/redisc-edit.mjs` edits rediscovery.json by text). Nothing on flip-reads to merge at either step.

| Step | State | Moved | Notes |
|---|---|---|---|
| 4, `outside` entries | `a98bc8e58` | 0 (no Rust) | 202, 214, 215, 217, 218, 219 carry ids now; new entries for ADR 0790's five roots and ADR 0773's sort bind/reflect; 15 reasons rewritten; two fused objects split; 48 `outside` of 250 left |
| 5, SC2001/SC4002 wording | `f1ed5195d`, ADR 0792 | 3 coverage rows, message only; sweep 24,659 U, 377 V unchanged | SC4002's open read claimed "no default; a provider exists" for every open key |

- Old builds in the VM (worktree `wt-<sha>p`, target `tgt-<sha>p` under /mnt/lima-colima/sc): `1232p` (1dd911b95, before ADR 0716: has none of 0688, 0716-0720, 0773), `79b99b4fbp`, `343bc76bbp`, `8f24d0154p`, `a92553232p`. The producer source is unchanged since each, so HEAD's `bin/solid-typefacts` serves them; not for ADR 0613 (ae9f51456^), which needs its own producer.
- 217's neighbours `.bind(st.a)()` and `Reflect.apply(sort, …)` were already uncertifiable at the start: R1's c129b2c29 (ADR 0773, merged at 8ff25dca5). The prep notes' "certified today" was r3base. No code in shared_reactivity.rs was needed.
- 174, 175 stay `outside`: their legal twins give no violation on the build before (0 of 1,512; 0 of 756). 185 stays: a parameter-after-await text variant is not-fired in Chrome (36 of 36), removed again.
- New live false clean: STRICT_READ_UNTRACKED `setter-fn-apply` with the `wrapper` binding, a store setter's callback read inside a body arrow an effect's apply calls (6 programs, same on e918518de). open-defects.json, open-work.md. Cause in SC1001's setter-callback placement (local_access.rs): not R3's file.
- Blocked: 198/199 (g2d) on local_access.rs `handed_role`.
- Same doubled clause left in SC1001's unplaced read message (findings.rs:467), out of step 5's scope.
- One mx-census smoke row moves better on every build since the merge (memo-shared false-clean to recall-gap): not R3b's, listed.
- `python3` heredocs hang in this shell; use node.

## Handoff at round 1 (part 8, integrator), 2026-11-06

Stopped at the owner's 500k rule. Part 8's order is finished except the census. A new agent takes the integrator's job and the census.

**Pushed head.** `claude/flip-reads` at `dd137e284`: 25,343 U, 377 V, 0 V moved against `f5fa5f5af` (24,515 U, 377 V), 0 exit errors, 0 unnamed, 92 open patterns (2 false positive, 90 false clean), `check.mjs all` 299 of 336, 37 kind clean, HOST 25 of 25. Holds R2 to `c6677ddac` plus item 7, R3 to `63c15d6df`, ADRs 0694, 0695, the composite walk fix, memory review items 2, 3, 5, 6, 8, the `focus.mjs` cap.

**Not in that head.**
- **R1, `8428ec497`, ADRs 0774-0779, including C6 and C1.** Dropped from round 1: `repro-causes` fails on `clean-interval-hook` (SC9012 `handed-member-read`, a hook's tuple accessor in JSX). Built alone it fails the same way, so it is R1's, not the merge's. `verify-delta` never selects `repro-causes`. R1b is on it in `t3-b3846b2a`. The fix is C1b's shape: an accessor that reaches only sync sources is proved clean. Do not relabel the case. Merge `claude/flip-r1` normally: this line was rebuilt without that merge, with no revert, so it is not a no-op.
- `claude/flip-reads-with-r1` (`cdc49ac7f`) is the old head with R1 merged, kept for reference. At that tree: 27,229 U, 377 V, the VM's number, patterns 91 open, witnesses 301 of 336. Do not push it.
- **R2's F1, `d2d45d470`, ADR 0782,** on `claude/flip-r2-wip`, not gated by its own message. The memos `fcf5abf1c` and `3fff4102a` stand on it and do not cherry-pick alone. Take the branch whole once R2 says F1 is ready.

**The integrator's job.** R1b and R2 commit to local branches, R3 pushes `origin/claude/flip-r3`, nobody else pushes `claude/flip-reads`. About once an hour: merge what they have, then one set of gates on the result, read every moved violation, push, report commit and totals. A branch that fails: find it, build it alone to prove it, drop it from the round, tell the owner, land the rest. Union `blockers.rs`, `rediscovery.json`, `open-defects.json`, `cases.json`.

**Scratch, which survives a reboot.** `/Users/thomas/dev/sc-scratch.noindex/flip-reads`. Never `/tmp`. All in sh, and these helpers exist because zsh does not split `$H`: `round.sh <tag> <prev-sweep>` is build, `patterns --update`, coverage, witnesses, HOST, sweep, diff; `covu.sh` updates snapshots; `v5.sh` is `verify-seq` writing a real exit to `t/vd-*.rc`; `mkverify.py` regenerates `verify-seq.sh` from `scripts/verify.sh`, so it cannot go stale; `union-red.py` unions `rediscovery.json`; `snapdiff.py` diffs snapshots row by row; `sweepdiff.py` lists every moved V with its source; `chk.sh` runs case dirs; `f.sh` runs a fixture; `gate-times.tsv` is the gate log. Baseline `sweep-base.json`, builds `rbase` (`f5fa5f5af`), `rm5` (this checker), `rr1` (R1 alone).

**Rules in force.** Every checker run: `sc-heavy /Users/thomas/dev/sc-scratch.noindex/sc-guard.sh 12 <cmd>`, including tests that start it. Report if the guard fires; it has not. One heavy check at a time, sweeps `--jobs 3`, release builds bare with `CARGO_BUILD_JOBS=6`, load under 30 first (`waitload.sh`). Never `nohup` past the guard. Do not call `make fuzz-patterns`, it builds inside the lock: call `patterns.mjs`. `check.mjs --jobs 6`. `focus.mjs` is capped at 6 now.

**The census, ADR 0692. Do not land it before R1b's C6 is in:** it sits in the same block of `reactive_analysis.rs` and trusts N1-N3, which are in.
- The revision is `/Users/thomas/dev/sc-scratch.noindex/flip-reads/v2/value_use.rs` (934 lines), with `v2/census-block.rs`. It has never been compiled. Base, byte-identical to the 09:31 saved output: `/Users/thomas/dev/sc-scratch.noindex/recovered/value_use.rs`.
- Applied there: the three bypasses of `endstage-report.md` 1c (`exported_directly`, the `++` arm dropped, `reference_use` exits answer `value-use-unclassified`), P11 (`Tag`, `Coerced` for holders), P12 (`returned` walks callers, `prop_reads`, `context_readers`, one hop), `created_use` for P13.
- Still to do: add `mod value_use;`; the block goes after `open_members` in `reactive_analysis.rs`, and `p-census.diff`'s `read_only` hunk; loop over creating calls for `created_use`, whose holder is no binding, using `creates_reactive_source`, `store` by primitive, `tuple` for a call read through `[0]`; compile.
- `primitive_results` holds only `++` and `--`, not the unary operators 1c says. Reading that report literally would be wrong there.
- `gen-values.py` and `hold/fixtures` are lost. The shapes are saved in `v2/value-uses.mjs.txt`. Rewrite the fixture `value-use-census`, with each form at module scope, in `export function`, and in `export default function`: a module-scope test proves nothing about the export arm.
- Confirmed in Chrome, certified before the census: `throw c` caught and called, at all three placements (scratch `ex1/`). Must end U: `throw`, `nested-operand`. `class-extends`, `computed-key` are quiet: they may go U.
- `hops` (scratch `lim1/`): a live false clean at every depth through `yield c`, handed to the census as its test.
- `check.mjs` needs a `closedBy` per the census note, not `status != certified`: an unrelated U must not pass a witness.
- Over-reach to guard: `Object.keys(st)` is quiet.
- Measure the rise and report it per project.

**Left of the bounds.** `value_reaches`, `may_run_in_leaf_owner_within` in `local_access.rs`, R1b's file: draft gone with `/tmp`, the audit text in `bounds-audit.md` holds the design; use the exact walk, a depth-limit answer of true moved 13 true V. `typed_sources` views and `context_origin` wait for the census.

**Open for the owner.** `flush(fn)` ruling done, ADR 0694; its `rediscovery.json` entry says no old build fails, because the judge rates both answers agree. Item 2 bounds depth, not fan-out: `sites^8` is the worst case left. `verify-delta` should select `repro-causes`: same hole as `tsc-oracle`, not done. `types/p3-cast-*` are R2's listed residual, for the owner.

**Traps from this session.** A merge conflict on `open-defects.json` that takes one side drops the other's entries; `--update` puts them back without their `item` text: restore it. `bld.sh` used to copy a stale binary after a failed build; it refuses now. A task's "exit 0" is the shell's last command: read the `.rc` file. Chain with `&&`: a failed `git switch` let a cherry-pick run on the wrong branch once. A script with `cat` last hides the real exit. `====` in a zsh command line is an error and kills the rest.
