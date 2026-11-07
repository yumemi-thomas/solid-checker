# gen-wip handoff (2026-11-07; NOT green)

## g4 = 50bb44efc measured (sc VM release, uncached Chrome on every row; gen-wip/g4-*.jsonl, v0850-all-ctx.jsonl)
- All 22 generator contexts (gstep-*, gen-yield-each/sync-each, gen-await..gen-drain; 954 programs): 97c1bb966 43 FP, 14 FC (12 Chrome-confirmed), 1 wrong-rule -> g4 0 FP, 0 FC, 0 wrong-rule. Chrome-differs left: only the pre-existing throw-in-flight family in untouched contexts (gen-await, gen-iife, gen-wrap, gen-delegate, gen-next, gen-prefix, gen-drain).
- gen cells (88): 0 FP/FC; the 4 throw-in-flight rows are now U (recall-gap/not-fired). lg twins (1,316): unchanged.
- Gates on g4: coverage (only intended snapshot moves, read) 0 after update, ownership-gate 0. Not yet: corpus sweep, verify-delta, make verify, ADR text; smoke-manifest stop still blocks rediscovery entries.

## Latest (Claude, after the Codex credit stop) — ffc77cfd0 + this commit
- Codex tree saved as 0ce3cd34a. Fixed its wrong blocker: `async-flight-error-unobserved` now only for a generator iterated in an async flight (`generator_flight_stepped`); owner-handed-inline GeneratorHanded back to function-run-unplaced.
- Debug build ok; coverage: only helper-prop-read (blocker callee-callback-timing -> function-run-unplaced, still U, accepted) and the new generator-step-owner snapshot; both updated in this commit, every row read.
- FIXED in g3 (write walk asks the full write walk at each step site, `write_roles.generator-steps-walked`; a possible resumption keeps a rejecting role Unknown): App.tsx:39 U, :54 and :65 V again; gen cells vs d1 differ only by the 4 throw-in-flight context reads V->U (ADR 0852 intent), census-local-g3.txt. Snapshot generator-step-owner must be re-updated (coverage).
- (was) OPEN precision loss: generator-step-owner App.tsx:54 (drained sync generator) and :65 (first segment) were SC2001 V on 97c1bb966, now U function-run-unplaced (fail-closed, but the README says negatives keep the compute's context). Trace with tools/arms-text.mjs before claiming green; likely the callee_roles `generator-steps` arm or write walk `generator-steps-unknown` for InPlace calls.
- Still not run on this tree: ownership-gate, VM census (gen cells, gstep, W1/W2 ctx, lg twins), corpus sweep, verify-delta, make verify, ADR 0851-0853 final text. The smoke-manifest stop stands (proposed patch in scratch; also the pending gen-yield-alias id).

## STOP before smoke manifest change (2026-11-07 continuation)

The brief requires a stop before changing a smoke manifest. No manifest, judge,
assertion, limit, allowlist, parity expectation, tsc-oracle case or count-ledger
change was made. Exact proposed manifest diff (NOT applied):
`/Users/thomas.guehenneux/EDF/tmp/flip-gen-await/smoke-w1w2-proposed.patch`.
Readable additions: `smoke-w1w2-additions.json` beside it. It appends 20 generated
W1/W2 ids (SC2001, owner cleanup, action scope, all four new contexts), their
expected behaviour indexes and missing behaviour strings to mx-census.json.
Reason: rediscovery.test.mjs requires every ledger id in the smoke manifest;
none of these contexts is currently held. Rediscovery entries cannot be added
and remain green without that change. The original brief says to stop here.
The later owner instruction authorizes saving measured WIP on flip-gen-wip.

### W1/W2 code and evidence
- W2 climbs in-place callbacks before asking the enclosing generator's step
  resumption. W1 checks exports both for generator bindings and held iterators;
  uses existing indexes::binding_exported (exact declaration/reference links),
  replacing the draft's name-only export-specifier comparison.
- VM release d7 SHA256: 400e3e3d5a0ec1954a492f547862653323bd62c65f42f13c8462a8f52e1ded5a.
  Build exit 0; semantic tree be69b79c1 + working diff (VM snapshot 967e96d82).
  Final host-only formatting wraps one expression; no semantic change.
- Four new contexts, 172 programs, every row uncached Chrome-confirmed on old
  v0850 (97c1bb966) and d7. Both matrix commands exit 0 (matrix reports rows;
  this is NOT a passing fuzz-focus claim). Guards: no memory trips.
  Old: 12 false-positive/same, 12 false-clean/same, 2 false-clean/not-fired-in-Chrome,
  8 agree/same, 2 agree/differs, 2 recall-gap/same, 134 not-fired/same.
  New: 0 judged FP/FC; 20 agree/same, 4 agree/differs, 2 not-fired/differs,
  2 recall-gap/same, 144 not-fired/same. See d7-w1w2-comparison.txt: every
  moved row was read. Confirmed: W2 writes/action/onFlush no longer V; 12
  Chrome-confirmed owner FC become missing-owner V. The 6 Chrome differences
  are NOT verified fixes: useContext/no-provider and dynamic-static throws in
  the async compute's flight (known issue already recorded below).
- W1 exported contexts are not executed by the runtime; their not-fired outcome
  alone proves no clean/U distinction. Historical local d6 output showed U for
  exported generator and exported iterator (cleanup, SC2001, action), vs old
  exported iterator's clean/V. A d7 raw-census run was queued for exact U/blocker
  evidence, then cancelled before acquiring the lock; another agent held it.
- make fuzz-test ARGS='scripts/fuzz/generate.test.mjs scripts/fuzz/rediscovery.test.mjs':
  exit 0, 16 passed, 0 failed/skipped. This validates the EXISTING ledger only;
  W1/W2 have no new ledger entry because of the stop above. git diff --check: 0.
- Deferred on d7: fuzz-focus/fuzz-smoke, fixtures/snapshot, coverage,
  ownership-gate, corpus, verify-delta, make verify, final ADR 0851. Existing
  base/final gate results below apply only to their named builds. NOT green.
- No own background or queued work remains. Original green branch still
  claude/flip-gen-await at 97c1bb966; continuation is WIP on flip-gen-wip.

## Blocking review findings on a84e8af61 (fix before any green commit; fuzz shape first, old fails, new passes, W2 in Chrome)
- W2 (plausible FP): `generator_step_resumption` asks only sites directly in the generator: `yield 1; [1].forEach(() => set(1));` under `for await` in a compute climbs to the compute's role: SC2001 V where Chrome is silent; owner rules go quiet the same way. Fix: climb in-place layers first (as `climbed_after_await` does).
- W1 (plausible silent clean): `generator-never-stepped` (Unreported) also answers for `export function* g()` / `export const it = g()` with no project reference. Exported or handed-out generator/iterator must be Unknown.

## Progress after a84e8af61
- d4 measured (sc VM, gen-wip/d4-*.jsonl): gen cells == d1; lg twins unchanged; other gen ctx unchanged; gstep: 31 FP -> 20 FP (all role-based), gstep-from back to agree.
- d5 (this commit, built, local only): write_role_walk stops at a generator covered by `generator_creations` (`write_roles.generator-steps[-unknown]`): gstep-handler SC2001/SC2002/SC3002 V -> U; gen cells unchanged (census-local-d5.txt). CHECK: gstep-spread writes became U (was V at v0850?) -- measure; spread should keep the body's role.


Base: claude/flip-gen-await 97c1bb966 (ADR 0850, pushed, green except pre-existing flaky backend_process/exec.test). This WIP is on top.

## What the code does (uncommitted ADR 0851, numbers 0851-0854 reserved)
- `execution_role::generator_step_resumption` (body_roles.rs): site past a dominating `yield` of a generator whose only uses are `for await`/awaiting `for…of` operands in async H → H's continuation (detached); `Some(false)` = Unknown. Wired into write_roles (`write_role_walk.unreported-generator`, suspendable) and owner_emission (after_yield / MayResumeUnowned). = build d1.
- `owners::iterator_steps` + `IteratorSteps {InPlace, Held(sites), Unplaced}`; owner_discovery: a call creating a generator's iterator gets no body edge; Held → edges from each step site (`call_owner_edge` there); Unplaced → source-less MayBeUnowned. = d2.
- `owners::generator_creations` + callee_roles arm `callee_roles.generator-steps` (role at the step sites; differ/unplaced → Unknown; never created → Unreported). = d3 (built, NOT measured).
- `array_from_source`: Array.from(g()) is InPlace. = d4 (built, NOT measured; fixes gstep-from CONTEXT_NOT_FOUND agree→recall-gap regression of d2).
- Fuzz: model.mjs GENERATOR_STEP_FORMS `gstep`, print.mjs, new flip-gen.mjs (10 `gstep-*` contexts, listed under MISSING_EFFECT_FN), misuse.mjs wiring. 430 census programs. Fuzz unit tests NOT run.
- Fixtures reactive-ir/generator-step-owner (d1 behaviour checked by hand, no snapshot yet), docs (rule pages SC2001/missing-owner, open-work line). No ADR text, no rediscovery entry yet.

## Measured (sc VM, release, uncached Chrome, every row confirmed) — files in gen-wip/
- mx-census gen-yield+gen-sync-loop (88): v0850 12 FP, 14 FC (2 Chrome not-fired); d1: 0 FP, 0 FC by the judge; 6 Chrome `differs` (throw-in-flight family below). lg-recall gen-prefix/gen-drain (1,316): identical, 0 FP.
- Other gen contexts (352): v0850 == d1.
- gstep-* (430): v0850 31 FP + 1 wrong-rule; d2: 11 owner FPs removed (unstepped-handler reverse case), NO_OWNER rows on handler/timer/kept/returned now missing-owner U; 20 FP left, all role-based (SC2001 x12, SC2002 x4, SC3002 x4 on handler/timer/kept/returned) — d3's arm targets them, unmeasured.
- Throw-in-flight family: useContext/no provider/dynamic static inside an async memo flight: V, fast host agrees, uncached Chrome not-fired (31 rows at v0850). Pre-existing; d1 adds 4 (context reads past yield). Not a judge change; reported.
- Corpus/coverage/verify NOT run on d1-d4 (coverage on d1: only the new fixture lacked a snapshot).

## Next
1. Build d4 under the shared lock; run tools census-ctx (gstep-*), census.sh (gen cells + lg twins), ctx (other gen contexts) on d4 vs v0850 (`vm.sh sync` first; one VM run at a time).
2. Read every moved row; then fixtures (generator-step-owner + gstep shapes), coverage-update, ownership-gate, sweep old/new (bins/new = 97c1 macOS release), verify-delta, make verify.
3. Rediscovery entries need ids in scripts/fuzz/smoke/mx-census.json (none of the gen/gstep ids are there): a smoke reselect = stop condition in the brief; report it, do not edit.
4. ADR 0851 text; open-work already has the known misses (gen-await own await, wrapper/yield* stepping, throw-in-flight).

## Traps
- `vm.sh sync` rebuilds the shared VM clone from the worktree: label binaries by what was synced (check sync-*.log time vs file mtimes).
- VM guard: per-checker 12 GB RSS + 85% of VM total (other agents' tmpfs counts in `used`).
- Shared lock: mkdir "$(git rev-parse --git-common-dir)/sc-heavy.lock", 30 s retry, never hold while waiting.
- make verify fails on base e57cc0dc5 too (backend_process "unchanged repeat is cached", exec.test 0.8 s).
