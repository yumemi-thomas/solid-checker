# gen-wip handoff (2026-11-07, stopped on account limit; NOT green, never built past d4, no gates on d3/d4)

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
