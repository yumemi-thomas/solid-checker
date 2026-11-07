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
