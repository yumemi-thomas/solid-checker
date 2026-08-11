# Pipeline orchestrator redesign

`build_with_contracts_measured_incremental` in
`rust/crates/solid-reactive-ir/src/pipeline.rs` is a ~1,000-line function
sequencing eleven analysis stages. The sequencing itself is sound; what makes
the function hard to read and risky to edit is that three cross-cutting
concerns are hand-rolled across every stage:

- **Timing.** `build_timings` is written by every stage, including from
  inside `thread::scope` closures; ~30 nested timing fields are copied by
  hand; two duplicated `finish_stage!` macros each need dead-store
  workarounds, and one stage hand-rolls its emission entirely.
- **Accumulators.** ~14 output vectors are declared in early stages and
  filled in later ones. `static_violations` is appended by five stages;
  stage 7 extends stage 5's `writes`; the returns-conditionally rule reads
  the fully merged `reads` vector. None of this is visible in a signature.
- **Reuse gates.** `late_stages_reusable` and friends are re-derived at five
  scattered points rather than decided once and carried.

Three stages already follow a clean extracted pattern — a read-only context
struct in, one cache slot, one owned result out, with the reuse decision
inside the stage (`discover_sources`, `LocalAccessContext::build`,
`InterproceduralContext::build`). The redesign applies that existing pattern
to the seven stages still inlined, behind three pieces of shared structure.

## Target shape

The orchestrator remains one function: the stage sequence, the two
`thread::scope` blocks, and the lifetime anchoring of the `owned_*` fallback
locals are genuinely orchestration and belong in one visible place. It
shrinks to ~120–150 lines of stage calls over:

**`StageClock` — one timing subsystem.** Owns the stage timer, the
`&mut BuildTimings`, and the `SOLID_CHECKER_TIMINGS` emission flag, with a
`finish(field, "stage-name")` method. Replaces both macro copies, the manual
timer re-anchors, and the hand-rolled `eprintln!`s. Nested-timings copies
become `BuildTimings::absorb_*` methods living next to the structs they copy
from. The emitted JSON line format and stage names are observable output the
performance tooling reads; they are preserved byte-for-byte and pinned by a
unit test.

**`ProgramDraft` — one owner for the accumulators.** Owns the output vectors
and obligation counters that are loose `mut` locals today, plus the
`seen_static` dedup set behind a `push_static` method. Stages take
`&mut ProgramDraft`, making cross-stage writes visible in signatures. Final
assembly becomes `draft.into_program(...)`, absorbing the ten sorts.

**A widened `StageContext` + `ReusePlan`.** The existing nine-field context
grows to carry what stages three through nine all share (entities, symbol
names, source declarations, reachable calls, rule options, the dialect),
built once after the reachability/discovery join. The early `mem::take` of
`resolved_contracts.missing_exports` moves into `ProgramDraft` seeding just
before the context is frozen. The reuse flags travel in one `ReusePlan`
value; the reuse *decisions* stay inside stages, matching the
`LocalAccessReuse` precedent.

Each inlined stage then becomes a function with the house signature
`fn stage(ctx, <cache sub-slot>, &mut draft, &mut clock)`: the three
static-prepass rules as separate functions in a new `static_rules.rs`, then
the upstream-compat block, leaf-and-cleanup, static-api, directives, and
returns-conditionally. The owner stage normalizes
`find_missing_owners_incremental`'s outlier signature (return timings, do
not also take `&mut BuildTimings`). The two thread scopes stay but their
lanes return `(result, timings)` for the orchestrator to absorb after the
join. Each `late_stages` cache sub-slot goes to exactly one stage as a
narrow `&mut`, making "owners is the last user of the slot" structural.

## Migration sequence

Each step lands as its own commit, independently green on the full oracle:
the workspace test suite, the reachability/owner/sessions parity suites,
`make coverage` (no finding moved), `make parity` (no deviation moved), and
zero-warning clippy.

1. `StageClock` + `BuildTimings::absorb_*` — mechanical; deletes both
   macros.
2. `ProgramDraft` + `into_program` — moves declarations and sorting.
3. `ReusePlan` + widened `StageContext` + the `missing_exports` seeding
   move.
4. Extract static-prepass into `static_rules.rs`.
5. Extract compat, leaf-and-cleanup, static-api, directives,
   returns-conditionally.
6. Restructure the two thread scopes to return timings; absorb after join.
7. Normalize the owner-stage signature; final orchestrator tidy plus a
   doc-comment stage map.

Finish with `make verify-performance` and a benchmark comparison: the
context structs are borrows and the draft owns what the locals owned, so no
new allocation is expected, but the timing restructure around thread joins
deserves measured confirmation.

## Known risks

- Split borrows on `ProgramDraft` (a stage reading `draft.reads` while
  appending violations): solved with draft methods or disjoint field
  borrows; the escape hatch is passing two fields explicitly.
- The timings emission contract: pinned by test, names unchanged.
- Behavior: guarded by the same move-only, oracle-gated discipline used for
  the fresh/incremental consolidation.
