# Fuzzer redundancy: what the runs say, with nothing changed

Job 3, 2026-11-05. Base commit 207b13142. Nothing was deleted and no generator was touched. Scripts: `/tmp/sc-scratch.noindex/fuzz-surface/j3/` (`load.py`, `load2.py`, `a2.py`-`a6.py`), results `axis.json`, `sib.json`, `cuts.json`, `sweep.json`.

## Answer

- **Safe to move to the weekly audit: 6 cuts, about 31,000 nightly recall programs (9% of the 350,000 lean recall grid), about 1.8 min of 20.** Control layers `do`, `label`, `switch`, `try-finally`; `module-barrel`, `module-star`; `depth-17`; `var-alias`; four member spellings; six namespace spellings. In every build their verdict matched the kept value's, including the builds where the verdict moved. A cut removes them from the nightly run only: the Sunday full audit keeps them.
- **The large savings are not supported.** `mx-combo` (2.1 min) differs on rc.21 builds. `lg-combo` differs on 3,065 programs. Half the pair grid has one full build of evidence.
- **The lean set holds on the only data there is, but that data is weak.** 0 of 563,208 demoted programs differed, on one build (audit 41775917761, before rc.21). No rc.21 run holds a demoted value. It was never rechecked on #173's code. Call: keep it demoted, unknown on rc.21.
- **A new risk the lean set carries.** `<Portal>` and `<Errored>`'s JSX forms are generated only by the weekly `--recall-full`. A default or nightly run has no `<Portal>` at all, and 7 rows of the state table are covered only there.

Saving is small because the money is elsewhere: per-value cuts here are thin slices. The default grids take about 10.6 min and lean recall about 20 min (sequential wall on the 14-core host). Pairs are about 23 min in full at the sampled rate.

## Data and method

- **Builds.** Every `programs.jsonl` under `rc21-open/out/` (104 directories, 4.3 M rows, 613,407 ids: full base runs `b0`, `g7`, `g9`, `g11`, `fin`, `g15`, `full1`-`full9`; lean recall `b0r`, `g7r`, `g9r`, `g11r`, `finr`, `r1old`, `r1new`; samples `premerge*`, `pairs*`; focused old/new pairs). Plus the full audit `fuzz-audit-41775917761` (1,768,439 programs, 2026-11-04, before rc.21 and before most of #173): the only full recall and full pair data.
- **Verdict.** A program's outcome plus its findings' `rule/kind`, line numbers left out (`redundancy.mjs`'s `answerOf`).
- **Neighbour.** The same program with one axis value replaced by the kept value, compared inside one build, never across builds.
- **Columns.** `differ`: programs whose verdict differs from the kept twin's in any build. `unique defect`: differs and this program has a `false-clean`, `false-positive`, `wrong-rule`, `tsc-overlap` or `disturbed` outcome while its twin does not, so the kept value would have missed it. `defect programs`: programs with such an outcome in any build, whether or not the twin has it too. `moved`: programs whose verdict changed between rc.21 builds, the evidence that the value was exposed to a change at all. A value with `moved` 0 and `differ` 0 has no evidence either way. `whole builds`: builds comparing at least 80% of the nightly programs.
- **Rediscovery.** Every `rediscovery.json` entry with `ids` (132 of 136): an id the cut removes is hit; an entry is lost when all its `ids` are hit. A partly hit entry keeps its other ids and the hit ids move to `fullIds`, which the test allows on recall grids.
- **Time.** Grid rates from `fin/perf.json` and `g11r` (ms per program): base 3.9, src 3.8, combo 4.6-4.9, recall 3.4; pairs 5.5 from `premerge9`, a sample. Sums do not remove overlap between cuts. Overlap is small, because recall crosses a layer with base bindings and a binding with base layers.

## Safe to move to the weekly audit

| cut | what | nightly programs | weekly-only or pair programs | nightly min | weekly min | differ | unique defect | defect programs | moved | whole builds | rediscovery ids hit / entries lost / entries partly hit | call |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| R1 control layers | do, label, switch, try-finally answered by while; keep while, for, try-catch | 12,744 | 20,561 | 0.7 | 1.2 | 0 | 0 | 192 | 732 | 8 | 7/0/2 | **safe**  |
| R2 module layers | module-barrel, module-star answered by module-default; keep module-default, module-ns | 6,372 | 10,274 | 0.4 | 0.6 | 0 | 0 | 108 | 377 | 8 | 0/0/0 | **safe**  |
| R9 depth-17 | depth-17 answered by depth-16 | 3,186 | 5,138 | 0.2 | 0.3 | 0 | 0 | 47 | 177 | 8 | 0/0/0 | **safe**  |
| R5 var-alias | var-alias answered by let-alias | 2,856 | 1,721 | 0.2 | 0.1 | 0 | 0 | 42 | 230 | 8 | 0/0/0 | **safe**  |
| R6 member spellings | computed-key, extracted, ns-default, ns-member answered by literal-member | 4,548 | 14,760 | 0.3 | 0.8 | 0 | 0 | 24 | 132 | 8 | 1/0/1 | **safe**  |
| R4 namespace bindings | six namespace spellings answered by ns-nested | 1,332 | 22,655 | 0.1 | 1.3 | 0 | 0 | 108 | 782 | 9 | 1/0/1 | **safe**  |

Totals: 31,038 nightly programs, 1.8 min. Mechanically this means adding `layers` and `bindings` lists to `LEAN_DEMOTED`, and moving the 4 partly hit entries' ids to `fullIds`.

These programs did find defects (`defect programs` is not 0), but always together with the kept value: `moved` is in the hundreds and `differ` is 0, so a change in the checker moved both. That is the strongest evidence these runs can give, and it is why these and not the values below.

**The caveat on R1.** The census found that an accessor **assigned** inside a block, `for`, `try`, `switch` or label is certified (`e03`-`e06`, `a25`). These layers wrap the **site**, not the assignment, so they have not been measured on that question. The new `reassign-*` paths must keep their own control forms, whatever happens to R1.

## Keep

| cut | what | nightly programs | weekly-only or pair programs | nightly min | weekly min | differ | unique defect | defect programs | moved | whole builds | rediscovery ids hit / entries lost / entries partly hit | call |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| R3 handoff layers | sort, tagged, reflect, apply, map, from answered by handoff-call | 19,116 | 30,827 | 1.1 | 1.8 | 209 | 10 | 324 | 1,134 | 8 | 236/0/3 | **keep** `handoff-map`, `-from`, `-sort`: 10 programs where the kept form was not a false clean (`f3a`). Same result as the review's warning: alike at a glance, not alike. |
| R7 show-later vs show-signal | show-signal answered by show-later | 19,116 | 31,267 | 1.1 | 1.8 | 0 | 0 | 318 | 1,105 | 8 | 1/1/0 | **keep** 0 differ, but it holds the only id of one entry (action call in an IIFE in a `createReaction` error member). |
| R8 legal mounts | legal grid only: for-item, dynamic, show-signal answered by show-later | 13,770 | 40,415 | 0.8 | 2.3 | 0 | 0 | 0 | 2,268 | 8 | 1/1/0 | **keep** 0 differ on 54,185 programs, and the same lost entry. The false positive of ADR 0570 came from `show-later` legal pairs, so this family has earned its place. |
| G2 lg-combo | whole grid, against the same program without the combination | 12,024 | 0 | 1.0 | 0.0 | 3,065 | 0 | 0 | 2,167 | 14 | 0/0/0 | **keep** 3,065 differ, and 2,167 programs moved between rc.21 builds: the most sensitive grid here. |
| P4 callback×handoff | callback×handoff: alias, shared answered by bundle | 0 | 32,511 | 0.0 | 3.0 | 4,792 | 0 | 0 | 5 | 1 | 330/4/1 | **keep** 4,792 differ, 4 entries lost. |

## Unknown

| cut | what | nightly programs | weekly-only or pair programs | nightly min | weekly min | differ | unique defect | defect programs | moved | whole builds | rediscovery ids hit / entries lost / entries partly hit | call |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| G1 mx-combo | whole grid, against the same program without the combination | 27,840 | 0 | 2.1 | 0.0 | 189 | 0 | 12 | 517 | 12 | 0/0/0 | **unknown** A combo program draws a mount and a source and adds extra sites, so its twin is the same mount and source without the extras, which exists for 23,987 of 27,840. 189 differ on rc.21 builds (for example recall-gap against agree on `settled-cleanup`), no unique defect, no rediscovery id. Against the plain base program it differs on about half, because of the drawn source. A combination that differs on rc.21 is a live signal. Worth 2.1 min per run. Rerun the comparison after #173 settles. |
| P1 pair file forms | handoff×file: drop +module-barrel, +module-star; keep +module-default, +module-ns | 0 | 49,233 | 0.0 | 4.5 | 0 | 0 | 0 | 82 | 1 | 10/0/1 | **unknown** 0 differ on 49,205 twins, but one full build, on old code. Rediscovery: 10 ids, 0 entries lost. The best pair candidate: try it after one full pair run on #173's final build. |
| P2 pair handoff forms | handoff×file: the eight array and call forms answered by handoff-call | 0 | 45,951 | 0.0 | 4.2 | 256 | 0 | 0 | 75 | 1 | 20/0/1 | **unknown** 256 differ (30 on rc.21 samples; for example recall-gap against agree). No unique defect. |
| P3 pair mounts | mount×source: for-signal, loading-on, for-item answered by show-later | 0 | 72,893 | 0.0 | 6.6 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown** 0 differ, **`moved` 0**: no build changed any of them, so this is no evidence at all. 6.6 min is the largest single saving and it has the weakest support. |
| S1 module-default-hook | module-default-hook answered by module-hook | 18,006 | 14,490 | 1.0 | 0.8 | 0 | 0 | 132 | 292 | 0 | 0/0/0 | **unknown** 0 differ, but twins for only 6,252 of 32,496 programs and no whole build. |

## The lean set (`LEAN_DEMOTED`), rechecked

`mapArray`'s `name` option, found in Job 1, arms the mapper's strict window (`dev.js` `mapArray`, `repeat`), so the `<For>` and `<Repeat>` mounts' equality with `<Show>` is an engine fact only for what `<For>` passes. It does not say anything about a hand `mapArray`, which no value here covers.

| cut | what | nightly programs | weekly-only or pair programs | nightly min | weekly min | differ | unique defect | defect programs | moved | whole builds | rediscovery ids hit / entries lost / entries partly hit | call |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| L show-fallback | lean set: show-fallback against show-signal | 0 | 48,612 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |
| L show-keyed | lean set: show-keyed against show-signal | 0 | 48,612 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |
| L for-signal | lean set: for-signal against show-signal | 0 | 48,612 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 1/0/1 | **unknown on rc.21**  |
| L for-unkeyed | lean set: for-unkeyed against show-signal | 0 | 48,612 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |
| L for-fallback | lean set: for-fallback against show-signal | 0 | 48,612 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |
| L switch | lean set: switch against show-signal | 0 | 48,612 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |
| L repeat | lean set: repeat against show-signal | 0 | 48,612 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |
| L loading | lean set: loading against the root program | 0 | 48,612 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 1/0/1 | **unknown on rc.21**  |
| L loading-on | lean set: loading-on against show-signal | 0 | 48,612 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |
| L errored | lean set: errored against show-signal | 0 | 48,552 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |
| L portal | lean set: portal against show-signal | 0 | 48,612 | 0.0 | 2.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |
| L cb function | lean set: callback function against const | 0 | 14,268 | 0.0 | 0.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |
| L cb let | lean set: callback let against const | 0 | 14,268 | 0.0 | 0.8 | 0 | 0 | 0 | 0 | 1 | 0/0/0 | **unknown on rc.21**  |

Every row: 0 differ against `show-signal` (`const` for callbacks; `loading` against the root program), 1 build, `moved` 0. The original measurement was on cd08cd408. This one is newer (audit 41775917761, 2026-11-04) and agrees, but it predates rc.21 and #173's mount changes (ADRs 0570, 0642 and the `<Dynamic>` and `clientOnly` work in the queue). No build after it holds a demoted value, because lean recall is the default and the only Sunday run I can read is that audit.

**Recommendation:** leave them demoted, and rerun `redundancy.mjs --against show-signal` on the first Sunday audit after #173 lands. Two rediscovery entries hold an id on a demoted value (`for-signal`, `loading`), each with others.

**Separate from redundancy:** `portal` is the only recall route to `<Portal>`. If the default grids should have `<Portal>` at all, one program belongs in a kept grid; `web.dev.js`'s `Portal` and `portalImpl` sites have no nightly coverage.

## Contexts and other sweeps

`sweep.json` lists 218 axis values identical to a sibling on at least 90% of their programs; 187 of those never had a defect outcome. The ones above are what I would act on. The rest are not cut candidates:

- **Contexts** cluster (17 of 21 in mx), but only because the base shapes agree in the same outcome. A context is the claim itself, so its verdict matching another's says nothing. Keep.
- **`direct`, `alias`, `destructured`, `hook`, `wrapper`** cluster through a few shared cells, and `direct` against `namespace` differs on 85. Keep.
- **`helper` against `module-helper`**: 73 differ. Keep.
- **`for` against `while`**: 22 differ, so `for` stays out of R1.

## What this method cannot see

- **A shape no value generates.** Everything in `missing.md` is outside these numbers. A cut measured here is only as good as the surface it was measured on: the D3 family was invisible to every run above.
- **The `untrack` warning.** The rediscovery entry for it (ADR 0628, `untrack` of a memo the body creates under `dynamic`) is `outside`: the page never settles, so no runtime record exists and it has no ids. The rediscovery check therefore cannot protect it. Those programs are out of `pairShapes` on purpose, and a fixture holds them. The lesson stands for R1-R9: equal verdicts on the row do not mean equal state, so the Sunday audit keeps every value and the cut stays reversible.
- **Secondary findings.** The comparison is by rule and kind, not by site. Two programs with the same rule at different sites compare equal.
- **`moved alone`.** I dropped it from the tables. On sampled builds it counts a twin missing from a sample as not moving, so it overstates.
