# Escaped values: WIP handoff (job 1 of the W carrier task, 2026-11-07)

This branch is **WIP and not green**: `make verify`, `verify-delta`, the full coverage cycle and the VM fuzz have not passed on this commit. The base is `origin/claude/flip-census` b40d5d6a7. The intended branch is `claude/flip-w-carrier`; this is `claude/flip-escaped-wip`.

## What is built (ADR 0860, package globals 0862; no ADR text written yet)

- `value_use.rs`: a new mode `Mode::Escaped { root, blocker }`.
  - Each open census position is queued: carrier, logical, member write, primitive-callback return, forwarded export, pattern past a hop, rest, parameter default (`escape`, `escape_if_here`).
  - The queue runs after the reference's own walk (`run_escapes`), on one separate step budget, so the reference's answer and rows are unchanged.
  - The escaped walk follows bindings, aliases, Gets, call results, literal holders, and std, constructor or unresolved calls (result, receiver, other arguments, callback parameters). It also follows project parameters, visible callers, global-object slots (`global_slot_uses`) and receivers back through `const y = x.self()` (`escaped_object`).
  - `site()` places an SC9012 row at each call, root Get or root write through the value, unless `read_execution_role` proves it legal there. A store write always gets a row.
  - A root's Get, and a pattern over a root, end the walk. A primitive argument is a use and also ends it.
- Escaped rows go into `ProgramDraft::deferred_defects` (`pipeline.rs`). They are pushed after every producer, because `push_defect` keeps the first row per (kind, path, start byte), and an earlier build let escaped rows displace other rows. That fix is compiled but **not measured**: the cycle was stopped. `reactive_analysis.rs` keeps them apart (`take_escaped_uses`), drops any that duplicate a pre-escape use's span, and applies `covered`.
- `interproc/dependency_values.rs`: `global_reads` now also records the installed keys (`Installed`). Each project call of `G.k()` or of the bare global `k()` in an untracked or unplaced role gets a `dependency-global-read` row at the call.
- Fixture `fixtures/reactive-ir/value-use-escaped`:
  - Positive: `Carriers`, with a read on a separate line for each escape.
  - Negative: `Tracked`, reads in a memo with no row.
  - The .gitignore lines for its stub are added.
- Snapshots updated with `coverage-update`, on the build **before** the deferred-defects fix: 175 rows added, 0 removed, all SC9012 uncertifiable. They must be regenerated and read again.
- `fixtures/ownership-cases/cases.json` case [86] `no-destructure/solid-v2/open-root/001` gains an SC9012 at 51..58 (`[{ a }]`). This is a pinned expectation added, not cleared. Tell the coordinator.
- Fuzz shapes in `scripts/fuzz/flip-read.mjs`, where the escape is at module scope and the read is on the site line:
  - Reads: `use-{map,logical,assign,each,global,untrack,self,store-map,store-nullish,hook-module}-apart`, with `legal: ["memo","effect-compute"]`.
  - Writes: `sw-{map,nullish}-apart`.
  - They land in **mx-recall** (lean): STRICT_READ_UNTRACKED 540 ids (body, effect-apply, root) and STORE_WRITE_OUTSIDE_SETTER 72 ids (handler, timer). In **lg-recall** there are 360 legal twins (memo, effect-compute).
  - Manifests are in `~/EDF/tmp/flipcarrier/ids/` (`mx-recall-STRICT_READ_UNTRACKED.json`, `mx-recall-STORE_WRITE_OUTSIDE_SETTER.json`, `lg-recall-STRICT_READ_UNTRACKED.json`; generator version, seed 1).
  - Regenerate them with the node snippets in the session (`recallShapes` / `recallLegalShapes` filtered on `-apart`, ids from `shapeId` / `legalId`), or with `rerun-ids.mjs`-style commands.

## Measured, and on which build

- The 18 fuzz patterns, with my scratch site checker `~/EDF/tmp/flipcarrier/tools/sites.mjs` (read lines found by hand):
  - Base b40d5d6a7 release: a row at the read in 0 of 18.
  - The change before the deferred fix: 18 of 18.
- Corpus (frozen rc.21, `--apps-root ~/EDF/Yumemi/GitHub/solid-checker-rc21-frozen/apps`), base against the change **before** the deferred fix:
  - V 294 → 294. U 42,111 → 44,146 (+2,139 / −104 by full key).
  - Every removed row still has a row on its line. The removals were the push_defect start-byte displacement, which the deferred fix targets: re-measure.
  - Added by blocker: value-to-unresolved-callee 1,160, value-forwarded 359, value-in-carrier 345, value-through-await 111. By app: app-game 911, kui 679, solid-groove 126.
  - Reports: `~/EDF/tmp/flipcarrier/sweep-{base,new}.json`. Compare them with `tools/sweepdiff.py`.
- Fixtures, on that same build: 175 SC9012 U added, 0 removed. Some duplicate a proved SC2003 V on the same line (`store-operation-may-not-write`, `store-sub-value-flow`), as the census's reference rows already did on base.
- Ownership gate: it failed only on case [86] before the cases.json edit. It has not been rerun since.
- Not run: the VM fuzz by id, `make verify`, `verify-delta`, `fuzz-patterns --update`, and the rediscovery entry. None of the 18 is in open-defects.json.

## VM script (`~/EDF/tmp/flipcarrier/tools/vm-run.sh`, copied below)

Launch it detached, under the shared lock, for example: `colima ssh --profile sc -- bash -lc 'setsid nohup bash /Users/thomas.guehenneux/EDF/tmp/flipcarrier/tools/vm-run.sh t1 > /mnt/lima-colima/sc/runs/carrier-t1.log 2>&1 < /dev/null &'`.

- It copies the host worktrees into its own `$R/carrier-{old,new}` and makes its own Linux builds.
- It symlinks node_modules from the shared clone, read only.
- It never touches the shared clone or `vm.sh sync`, which writes `refs/fuzz-vm/sync` in the shared host repo.

```bash
#!/usr/bin/env bash
set -uo pipefail
R=/mnt/lima-colima/sc
H=/Users/thomas.guehenneux/EDF/tmp/flipcarrier
tag=${1:?tag}
export PATH=$R/node/bin:$HOME/.bun/bin:$HOME/.cargo/bin:$PATH SOLID_CHECKER_TSRS_BIN=$R/tsrs/target/release/tsrs TMPDIR=$R/ram/carrier-tmp SOLID_CHECKER_FUZZ_SCRATCH=$R/ram/carrier-scratch
export CARGO_BUILD_JOBS=12
mkdir -p $TMPDIR $SOLID_CHECKER_FUZZ_SCRATCH
out=$R/runs/carrier-$tag; mkdir -p $out
log() { echo "$(date '+%F %T') $*" | tee -a $out/status; }
( me=$$; while kill -0 $me 2>/dev/null; do
    for p in $(pgrep -s $(ps -o sid= -p $me | tr -d ' ') 2>/dev/null); do
      rss=$(awk '/VmRSS/{print $2}' /proc/$p/status 2>/dev/null); [ -n "$rss" ] && [ "$rss" -gt 12582912 ] && { echo "guard: kill $p rss ${rss}kB" >> $out/status; kill -9 $p; }
    done; sleep 2; done ) &
for b in old new; do
  src=$H/wt; [ $b = old ] && src=$H/base-wt
  dst=$R/carrier-$b; mkdir -p $dst
  rsync -a --delete --exclude /rust/target --exclude /bin --exclude /.git --exclude 'node_modules' --exclude /target $src/ $dst/
  for dir in scripts/fuzz/install scripts/runtime-oracle-install packages/cli; do
    [ -e $dst/$dir/node_modules ] || ln -s $R/solid-checker/$dir/node_modules $dst/$dir/node_modules
  done
  log "build $b"; (cd $dst && make -s build-checker-release > $out/build-$b.log 2>&1) || { log "build $b failed"; exit 1; }
  sha256sum $dst/rust/target/release/solid-checker-rust | tee -a $out/status
done
gen=$R/carrier-new
for b in old new; do
  bin=$R/carrier-$b/rust/target/release/solid-checker-rust; tf=$R/carrier-$b/bin/solid-typefacts; cd $gen
  node scripts/fuzz/matrix.mjs --grid --recall --seed 1 --codes STRICT_READ_UNTRACKED --contexts body,effect-apply,root --ids $H/ids/mx-recall-STRICT_READ_UNTRACKED.json --out $out/$b/mx-SRU --confirm-with-chrome --checker-bin $bin --typefacts $tf > $out/$b-mx-SRU.log 2>&1; log "$b mx SRU rc=$?"
  node scripts/fuzz/matrix.mjs --grid --recall --seed 1 --codes STORE_WRITE_OUTSIDE_SETTER --contexts handler,timer --ids $H/ids/mx-recall-STORE_WRITE_OUTSIDE_SETTER.json --out $out/$b/mx-SWOS --confirm-with-chrome --checker-bin $bin --typefacts $tf > $out/$b-mx-SWOS.log 2>&1; log "$b mx SWOS rc=$?"
  node scripts/fuzz/legal.mjs --grid --recall --seed 1 --codes STRICT_READ_UNTRACKED --contexts effect-compute,memo --ids $H/ids/lg-recall-STRICT_READ_UNTRACKED.json --out $out/$b/lg-SRU --confirm-with-chrome --checker-bin $bin --typefacts $tf > $out/$b-lg-SRU.log 2>&1; log "$b lg SRU rc=$?"
done
log done
```

How to judge the run:
- On the old build the `-apart` programs should come out `recall-gap` with `detail.remote`: an SC9012 on the escape's line, not the site's. The 4c site-aware judge makes those false cleans.
- On the new build they should come out `recall-gap` without `remote`, or `agree`.
- The legal twins must show 0 false positives.

## Update (build with the deferred-defects fix, same tree as this commit)
Measured on it: the 18 patterns have a row at the read in 18 of 18. Corpus base against the change: V 294 → 294, 0 rows removed, +2,035 SC9012 U (value-to-unresolved-callee 1,146, value-forwarded 333, value-in-carrier 282, value-through-await 111; app-game 900, kui 629, solid-groove 99, compass-ui 78, expenses-app 68). Fixtures: +175 U, 0 removed; snapshots regenerated on this build. Still owed: the ownership gate rerun, the VM run, verify.

## VM run t1 (2026-11-07, sc VM, own dirs `carrier-{old,new}`, generator of this tree)
Ownership gate: passes (96 cases), with the case [86] edit. Each program was confirmed in Chrome.
- mx-recall STRICT_READ_UNTRACKED (540 `-apart` ids): old b40d5d6a7 has 270 `recall-gap` with `remote` (an SC9012 only on the escape's line, a false clean under the site judge) and 270 at the site. New: 540 at the site.
- mx-recall STORE_WRITE_OUTSIDE_SETTER (72): old has 36 remote and 36 agree. New: 36 at the site and 36 agree.
- lg-recall (360 twins): no false positive on either build. Old 144 U / 216 clean, new 156 / 204. The 12 moved are all `use-each-apart` `wrapper` twins (the forEach callback's role is not proven tracked), clean to U.
- The rediscovery entry is added, with the reason the d8 package shape is not generated. Outputs: `~/EDF/tmp/flipcarrier/vm-t1/`.

## Next steps

1. Rebuild. Rerun `sites.mjs` (still 18 of 18) and `coverage-update` against the before-snapshots (`~/EDF/tmp/flipcarrier/snap-before`, `tools/snaptext.py` prints each row with its source line). Read every moved row.
2. Rerun the sweep of the change and compare with `sweep-base.json`: expect V unchanged and no removed rows after the deferred fix. Then the ownership gate.
3. The VM run above.
4. Add a `scripts/fuzz/rediscovery.json` entry (ADR 0860) naming the `-apart` ids. Note that the d8 package-global shape cannot be generated: the grid has no package fixture. Run `fuzz-patterns`.
5. Write the ADR 0860/0862 entries in decisions.md (keep within its budget), docs/open-work.md, and the rule page.
6. Run `make verify-delta`, then `make verify`. Commit green on `claude/flip-w-carrier`. Job 2 (the count.mjs ledger and judge.mjs vanishedRows) is not started.

## Traps

- The shell has noclobber on: overwrite with `>|`.
- `withlock.sh` (`~/EDF/tmp/flipcarrier/tools`) takes `.git/sc-heavy.lock`. Never nest it.
- A fixture stub must be `git add`ed before coverage accepts it.
- The coverage diff is positional: use `snaptext.py`.
- `push_defect` dedups by start byte, so order decides which row survives.
- The d8 fixture route gives SC9005 (package source unavailable). Only the pattern lane exercises package globals.
- Base binaries: `~/EDF/tmp/flipcarrier/base-wt` (b40d5d6a7, which has the memory fix e40429f5e).

## Continuation (Codex, 2026-11-07)

- `make verify` attempt `verify2.log` exited 2: builds-and-suites failed. Rust: 155 passed / 3 failed, all unchanged-repeat cache-hit assertions. Fuzz: 1098 passed / 4 failed: two executor startup deadlines, rewrite typecheck timeout, and one rediscovery code mismatch. No limits or assertions changed.
- Split the rediscovery entry by runtime code (read ids under STRICT_READ_UNTRACKED, write ids under STORE_WRITE_OUTSIDE_SETTER). Added the escaped-read and escaped-write precision notes to their rule pages.
- Re-ran `a_redirected_types_main_or_exports_field_is_resolved_again` alone under the heavy lock and 12 GB guard: exit 0 (1 passed). Router retries are queued behind another agent's heavy job.
- Next: router retries; isolated executor/rewrite/rediscovery tests; verify-delta; verification handoff. Job 2 remains untouched.
- The Next steps above predate the update and VM sections: sites, sweep, snapshots, ownership and VM t1 were already measured, as recorded there. Do not redo those without a changed semantic build.

- Count ledger preflight found the snapshot rise had not been recorded. `node scripts/guard/count.mjs --update` records the measured +175 SC9012 rows (11 existing keys); no drops or renames changed. The gate now exits 0: 8,108 U rows / 189 keys. This records Job 1 results; the Job 2 gate implementation is unchanged.

- Isolated retries under lock/guard: router module exit 0 (4 passed), executor exit 0 (4 passed), rewrite exit 0 (26 passed, typed profile sample 82 seconds), rediscovery exit 0 (2 passed).
- `BASE=b40d5d6a7 make verify-delta` exited 2 at ir-read-lint, before build/test checks: new callback file lookup, global file scan, and a `files` variable falsely recognized as the census HashMap. No allowlist changed. Callback lookup now uses the existing recorded `place` helper; global scanning replays `slot_entries(base_members())` with a SourceTexts memo read for every file. Lint and rustfmt pass; a new verify-delta is running. Previous semantic measurements precede this recorded-read-only edit.

- Delta4 reached clippy and found a compile error in the recorded lookup refactor (`Span::location` needs `&str`, not `&String`). Corrected to `path.as_str()`; delta5 is running. No allowlist, judge, assertion or limit changes.

## Verification continuation (2026-11-07, awaiting assertion approval)

- `BASE=b40d5d6a7 make verify-delta` (delta5): exit 0, all selected checks passed. Coverage: 891 projects, 10,429 findings, no mismatch. Ownership: 96 cases / 465 ledger rows, zero pending. Fuzz unit suite: 1,102 passed / zero failed or skipped. IR library: 352 passed / one ignored. IR edit fuzz: zero mismatches, including 16 retype steps. Count ledger, lint, clippy, WASM compile, published-type oracle, obligations and recorded witnesses passed.
- Full `make verify` (verify3): exit 2 at builds-and-suites. A package-reinstallation repeat-cache-hit assertion failed under load (157/158 backend tests passed), and one executor startup exceeded its unchanged 0.8 second deadline (1,101/1,102 fuzz tests passed). Isolated reinstall retry: exit 0.
- Full retry with `RUST_TEST_THREADS=1` (verify4): exit 2 at builds-and-suites. All 158 backend process tests and all 1,102 fuzz tests passed. The later diagnostics suite exposed two real stale expectations: forwarded-tracked-compute QueryOptions rows 6 -> 8 (SC9012 value-forwarded, options() at bytes 369 and 627); returned-store-proxy App rows 4 -> 17 (12 value-through-new, one value-in-carrier). The snapshots already contain all these additions. No violation assertion failed.
- Exact proposed assertion diff: `~/EDF/tmp/flipcarrier/assertion-proposal.patch`, NOT APPLIED. Asked the user for the explicit approval Job 1 requires before changing assertions. Existing site assertions remain; proposal also pins the new options() U rows. No further build dependent on that change until approval.
- Final release rebuild/site/sweep script did not run: it stops on failed make verify. Previous saved semantic measurements still precede the recorded-read-only refactor.
- Job 2 clarification approved by user: fixtures.proved must contain the key at baseline and have a lower current count; fixtures.open must contain it currently. No Job 2 implementation yet.

- User approved the proposed Job 1 assertion update ("Do what is correct and recommended"). Applied it and rustfmt; no original per-read or violation assertion removed. The diagnostics suite is queued/running under lock and guard. Full verification must be rerun on this approved tree.

- verify5 (12:24) failed at gates: wasm-parity w3-dependency-source-program metrics 5/2 -> 6/3 proof/unresolved obligations, finding rows and native/WASM agreement unchanged; and the CLI development-feedback timeout test (time only). Disposition (inferred, not run separately): the added obligation is ADR 0860's escaped-value row inside dep-hooks, whose package-located findings the project report drops (diagnostics.rs:412). Recorded 6/3 in expectations.json under the user's approval of count-expectation updates. Next: rerun make verify once; the timeout test alone if it fails again.
- verify6 on 380030b0a (13:12, RUST_TEST_THREADS=1, 934 s): exit 2, and the only failure was the js lane's `development-feedback.test.mjs` "a timeout returns a structured partial result" (time only). wasm-parity passed with 6/3; the fuzz suite passed 1,102/1,102; ir-edit-fuzz had 0 mismatches. The test was rerun alone twice under the lock: 46/46 passed both times. No limit changed. Job 1 is green apart from that time-only failure. Owed: land it on claude/flip-w-carrier (squash or cherry-pick of this WIP series, coordinator's call); Job 2.

## W3 (review of 7c0eff2d9), in progress
- Fix 1, `escaped_object` silent ends: past MAX_ALIASES it leaves a use row. On a written object that is no name, it follows what the called project function returns (`escaped_returned_object`), else it leaves a use row. Probe `~/EDF/tmp/flipcarrier/probe/a`, a store written into a call's result (`hb().s = st`) and read as `h.s.a` in a body: old 7c0eff2d9 has no row at the read; new has one (value-held-in-member). The fuzz shape `use-call-target-apart` is now that store form. The VM run of the first (signal) form agreed on both builds, so it was not a reproduction.
- Fix 2, `site()` clears a DeferredCallback read only on a proved fresh stack (`fresh_stack_callback`). **No measured effect.** The `use-setter-apart` shape (effect-apply) is rowed on both builds by another producer. Probe `e`, an escaped `??` accessor read inside `setSw((d) => …)` in a component body, has **no row at the read on either build**. The cause is not found: candidates are a read row there that `covered` drops but the projection never reports, or `site` never being reached. Chrome is not measured for `e`.
- VM run w3 (`vm-run2.sh`, old 7c0eff2d9 / new): the earlier `-apart` results are identical on both builds and legal twins have no false positive. The W3 ids gave the same results on both (the shapes did not isolate a gap).
- Not run yet for W3: coverage, sweep, verify.
