# M0 state: the writer-cause census and what-if (ADR 1114 family 3; ADR 1131 re-scope)

Mac 20b, stopped 2026-11-20 14:50 by the coordinator (owner caps local implementers at 2); M0 continues on the W machine.
Report: `M0-results.md` (all tables; every writer of every project: `M0-writers.md`), both in this directory. Method text: `M0-method.md`. Found on the way: `found-on-the-way.md`.

## Branch, builds, what moved nothing

- Branch `claude/writer-cause-census-m0` (origin `yumemi-thomas/solid-checker`, never main). Rust is final at `ba9057061` (the switch is reachable only in the `what-if` cargo feature build, ADR 0950); every later commit is scripts or docs. Head = the commit that adds `scripts/writer-census-steps/` (named in the reply). Measurement only: no verdict changes by default.
- Base = `dba9d3cee` (main with ADR 1114 family 3 slices A-F). Main has moved since (a64fc92ad: #296 ADR 1125, #298 ADR 1134, ADR 1142 on its own branch): the numbers below are on `dba9d3cee`, not on today's main.
- Worktrees here: head `/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0`, base `/Users/thomas/dev/sc-worktrees.noindex/mac20b-m0-base` (detached `dba9d3cee`; remove it when M0 is done: `git worktree remove`).
- Pinned binaries (scratch `pin/`): `checker-base` (dba9d3cee), `checker-head` (7f48f8573: the first form, the switch ungated: the measurement runs of pipelines 1-2 used it), `checker-final` (bc06db9d2: the default build, the switch gated), `checker-whatif` (bc06db9d2 with `--features what-if`), `typefacts` (the producer, unchanged between dba9d3cee and a64fc92ad). `pin/head.sha`, `pin/final.sha` name the commits.

## What is measured (all on the frozen rc.21 corpus v2: 34 projects, `--runtime-target browser --runtime-build development --rendering csr`, release builds)

| claim | number | how |
|---|---|---|
| family-3 rows (SC2002, `analysisContext` holds `:prototype-`) | 146,845 (the design report counted 136,756 at slice E; this head carries slice F) | census run, `runs/census-v2` |
| writers | 629 roots = 349 writer packages (app-game 197 roots, 85 packages); every one non-clear, every one has a recorded cause | census |
| the writer side can move at most | 98,436 rows (67.0%); 48,409 stay with every any-name writer left out (the floor) | floor run per project |
| cause-level mechanisms M1, M1w, M1u, M3, X1131, M3u, M4, M4w, M5b, M5, M5u, M6 | 0 rows each, and 0 together (upper bounds included) | offline decision checked by runs; a project's rows go only when its LAST any-name writer is decided |
| M7v (reviewed bytes of vite's injected dev client) | 3,319 rows, 2 projects (donegeon-client, helge-dev), no ruling | scenario run |
| M2a (a package no value import loads) | 0 rows | scenario run |
| M2b (the product realm; needs the owner's ruling) | 22,941 rows in product files, 25,751 counting every row of the freed projects, 13 projects (kui's eleven, every-deck-of-cards, jar-hell-web) | scenario run (a file-name test of the importers: a measurement tool, not the real entry-set mechanism) |
| the ranked plan M2b then M7v | 26,261 rows in product files, 29,071 counting every row of the freed projects (19.8% of 146,845), 16 projects; without the ruling only M7v: 3,319 (2.3%) | scenario run |
| against the design report's ceiling | estimated "about 36,900 of 136,756 (27%) with the test-and-config ruling, 0 to 1,800 without" -> measured 29,071 of 146,845 (19.8%) with it, 3,319 (2.3%) without | |
| ADR 1131 alone | 0 rows (X1131 decides every source/view/defaultView route in a file of the program: 110 of 17,446 route occurrences are in program files; app-game: 57 in 31 files; the coordinator's "139 routes in app-game's own files" is the same reads counted once per spelling) | census + scenario |
| validation | the offline decision agreed with the checker's runs in 1088 of 1088 (scenario, project) pairs; V2 (all but one writer left out) keeps every row in 32 of 32 projects with two or more writers; the checker's own class matcher (`SOLID_CHECKER_WHATIF_WRITER_CAUSES`) left out the same roots as the offline step in all 629 roots (0 differences) | |
| switch off by default | default build `bc06db9d2`: 33 of 34 projects byte-identical to the base (stdout sha256), also with the switch variables set; app-game differs only by the base's own nondeterministic SC5003 `relatedLocations` (the base differs from itself; with SC5003 left out it is identical). The what-if build with no switch equals the base apart from the `assumed` taint (34 of 34); with the switch it reproduces the scenarios in 169 of 170 (scenario, project) pairs (the 170th, M7v on app-game: 3 SC5003 rows of `geometry.ts`, family 3 equal) | `runs/final-off`, `runs/final-env`, `runs/whatif-off`, `scen-whatif/` |

## What is NOT measured (left for W)

1. **Step C (identity on programs the corpus does not hold): not run.** monotone M1 with a no-op ablation on the textbook, M2 on textbook + fixtures, `pin/checker-base` against `pin/checker-final`; expected 0 moves / identical findings. It waited in the shared heavy queue and was stopped when the cap came. Run: `sc-heavy scripts/writer-census-steps/stepC.sh` (about 2 minutes of work; outputs `runs/monotone-textbook-m1.json`, `runs/monotone-fixtures-m2.json`; exit 0 = identical).
2. **Step D (optional): not run.** The design report's own columns A, A+esc, B, B+esc as checker runs (they are in the report as offline predictions from the validated model, not as runs), and the production-build readings (`--runtime-build production`: std mode) with a production-build census (`runs/census-std`). Run: `sc-heavy scripts/writer-census-steps/stepD.sh` after step C (one heavy job at a time); then `RUNS=<scratch>/runs node scripts/writer-census-steps/std-compare.mjs`.
3. M2b is a file-name test, not the entry-set mechanism (ADR 1055 part 2, ADR 1079 step 0); the owner's ruling on realms is pending. M-sizes (S=1, M=2, ...) and the design report's figures are estimated, not measured.
4. `make verify` was not run (the branch adds `scripts/` paths, which escalate verify-delta to the full verify). `make test-what-if` does not compile at the base (`what_if_filter.rs:67`, untouched by this branch): lint the lib and the bins of the feature build only.
5. No ADR number is taken for the switch (the coordinator assigns it: ask before writing one).

## Scripts (all in the branch)

- `scripts/writer-census.mjs`: the corpus driver (one checker process per project; `--census` turns the census on, `--drop-dir`/`--whatif-causes` set the switch, `--keep-report`, `--runtime-build production`); exports runProject/saveResult/isFamily3.
- `scripts/writer-census-report.mjs`: the offline library (`describe`, `dropRoots`, the mechanism definitions `MECHANISMS`) and `drops` / `crosscheck` commands.
- `scripts/writer-census-scenarios.mjs`: runs the checker with drop lists per scenario, keeps what moved (`--only` names scenarios; runs are cached by project and drop set).
- `scripts/writer-census-tables.mjs`: the tables (`tables.md`, `writers.md`, `results.json`); `--emit-scenarios F --stage A|B|C` writes the scenario lists (`--assume-floor-zero 1` for stage B).
- `scripts/writer-census-identity.mjs`: byte identity of runs against the base (`--base DIR --head DIR [--other DIR]`).
- `scripts/writer-census-steps/` (new in the head commit): the step wrappers I ran, with `HEAD`, `BASE`, `S` overridable by environment (`S` = the scratch root holding `pin/`, `runs/`, `scen/`, `tables/`): `pipeline1.sh` (build head, census-v2, identity sweeps), `pipeline2.sh` (all scenarios + the base's second app-game run), `stepA.sh` (final default build + identity checks), `stepB.sh` (what-if build + checks), `stepC.sh`, `stepD.sh`, `scen.sh`, `identity.sh`, `pipeline3.sh` (older form of step C), `whatif-compare.mjs`, `std-compare.mjs`, `assemble.mjs` (builds `M0-results.md` from `tables/final`), `M0-method.md`, `found-on-the-way.md`, `identity-summary.part1.txt`.
- Rust (solid-facts: `module_reach/writer_causes.rs`, `module_reach.rs`, `package_reach.rs`; solid-facts-backend: `package_reach.rs`, `check.rs`): the census (`SOLID_CHECKER_WRITER_CENSUS=1`, stderr JSON, any build) and the switch (`SOLID_CHECKER_WHATIF_WRITER_DROP=<file of roots>`, `SOLID_CHECKER_WHATIF_WRITER_CAUSES=<class prefixes,unloaded>`; the `what-if` feature build only, the default build ignores them).

## How to run each step (W machine, in order)

Prerequisites: the branch checked out (head worktree) and a second worktree of `dba9d3cee` (base); the frozen corpus (`solid-checker-rc21-frozen-v2/apps`, `--apps-root` if it is not at the default path of `scripts/lib/corpus-manifest.mjs`) with `scripts/corpus/rc21-manifest.json`; `bin/solid-typefacts` (`make build-typefacts`); the shared heavy-lock wrapper (`sc-heavy`) around every build and corpus step, one at a time. Scratch layout: `export S=<scratch dir>` (`pin/`, `runs/`, `scen/`, `tables/` are created).

1. (only if the runs are not copied from the Mac) `sc-heavy scripts/writer-census-steps/pipeline1.sh`: release build of the head, pin it, census-v2 (the baseline the tables read), identity sweeps of base and head. About 4 minutes of work. NOTE: the scenario runs of the report used the first form of the build (switch ungated, `checker-head`); on the gated branch head run scenarios with the what-if build: `make build-what-if`, then `--checker rust/target-what-if/release/solid-checker-what-if` (copy it to `pin/checker-whatif`), and edit `pin/checker-head` to that in `pipeline2.sh`/`stepD.sh`/`scen.sh`.
2. `node scripts/writer-census-tables.mjs --baseline $S/runs/census-v2 --emit-scenarios $S/tables/v2-pre/stageAB.json --stage A` (then `--stage B --assume-floor-zero 1`, then `--stage C` for step D): the scenario lists.
3. `sc-heavy scripts/writer-census-steps/pipeline2.sh`: every scenario (floor, V2, the class matcher, every mechanism alone, the plan's sets) and the base's second app-game run. About 9 minutes.
4. `sc-heavy scripts/writer-census-steps/stepA.sh` (final default build, byte identity with nothing on and with the switch set), `stepB.sh` (what-if build, no switch against the base, the headline scenarios again), then **`stepC.sh`**, optionally `stepD.sh`. Each is one job, a few minutes.
5. Tables and report: `node scripts/writer-census-tables.mjs --baseline $S/runs/census-v2 --scenarios $S/scen --out $S/tables/final`; write `$S/runs/identity-summary.txt` (start from `identity-summary.part1.txt`, add the what-if build and step C results); `cp $S/tables/final/writers.md $S/M0-writers.md`; `S=$S node scripts/writer-census-steps/assemble.mjs $S/tables/final $S/M0-results.md` (it also reads `$S/M0-method.md`, `$S/found-on-the-way.md`, `$S/pin/*.sha`).
6. Push the branch (never main), then reply to the coordinator: measured vs estimated, the numbers above, branch + SHA, the recommendation (below).

## What I recommend building first

1. Ask the owner for the M2b ruling (writers only test and config entries load): it is a decision and the only lever with mass (22,941 product rows, 25,751 with every row of the freed projects, 13 projects). The real mechanism needs the entry set.
2. M7v (reviewed bytes for vite's injected dev client) is the only no-ruling item: 3,319 rows, 2 projects.
3. Do not build M1, M3, M4, M5, M6 or ADR 1131 for family-3 rows: they free 0 rows alone or together, because the last writer of every project holds unread code, real generation or prototype-key writes. After both steps 117,774 rows (80.2%) remain: those move only at the site.
4. Queue the scan finding (`found-on-the-way.md` item 1: a global in an export position, `export default globalThis`, is invisible to the module scan; a false-clean candidate with a probe at `$S/probe-export-default`).

## Rules in force (unchanged)

Never push main, no force-push, no reset --hard / checkout -- / restore / clean, no --no-verify, kill only your own PIDs, every heavy step through the shared lock wrapper (one job at a time, ~15 minutes max), cargo checks through the build lane, release binary for corpus runs, hand runs inside the `scripts/fuzz/install` layout, zero false positives and false cleans, ask the coordinator for any ADR number.

## ADR 1142 (the priority job that interrupted M0) is done

Branch `claude/carried-handler-owner-context-1142` @ `870c0b4d2`, verify-delta green, reported to the coordinator (memory: `adr1142-carried-handler-state.md`).
