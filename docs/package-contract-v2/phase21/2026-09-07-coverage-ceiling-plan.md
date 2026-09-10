# Search and implementation plan for the remaining coverage ceiling

The active objective is to investigate and implement all supportable coverage
improvements, preserving exact consumer proof and existing accepted cases.
The ceiling is not established by a flat count after one slice.

Latest full baseline: [2026-09-08-initial-reads-full](2026-09-08-initial-read-full-frontier.md),
418 probes, 318 complete, 61 partial, 30 refused, 9 not advanced, and 1,459
accepted artifact selections. Motion's recovered cases are preserved, but 30
other selections are lost across ten rows. Prioritize positive first-read
evidence for Marker (three complete rows) and then the conditional/loop origin
gaps and graph-context identity issue. None is an estimated certification gain
until a fresh published catalog and ordinary consumer establish it.

ADR 0066's subsequent [seven-probe run](2026-09-08-initial-receiver-recovery.md)
now recovers all three Marker roots and refused-to-complete transitions,
preserving all ten Motion/Utils selections and claims. Full verification passes
(243.12s); the full corpus has not been rerun for this slice. Move next to
conditional/loop origins and exact compiler program contexts. The remaining 27
losses from the full baseline have not yet been remeasured.

**Current full baseline: [2026-09-08-receipt-composition-full-v2](2026-09-08-receipt-composition.md),
418 probes, 324 complete, 62 partial, 23 refused, 9 not advanced, 1,152
certified entrypoints, 371 rows with the root certified.** It adds ADRs 0071 and
0072: one row changes (`solid-js@2.0.0-rc.3`, 1 → 2 entrypoints with its root)
and 417 are identical.

Receipt composition is **done and measured, and it was not the lever it looked
like.** Composition already happens inside a probe, over the exact installed
dependency, in dependency order; a cross-row catalog cannot replace it, because
only three of 121 dependency identities are certified corpus rows at the exact
installed version. What was missing was reach and granularity: the composing
lanes were requested by a reviewed probe-id list rather than by the row's own
refusal census (ADR 0072), and one broken graph node refused every artifact
case in its transaction (ADR 0071). Frontier cases reaching publication went
**61 → 65 of 173**. The 108 that remain are now attributed exactly, and none is
a composition gap — 39 are Type Facts proof refusals inside a graph that
composed correctly, 28 sit above a measured 32-case recovery memory budget, 24
reach a Node builtin, 7 a broken transitive install. The self-package edge
(`solid-js/web` re-exporting `"solid-js"`, 11 cases) is **not** a next step
here: [ADR 0012](../../adr/0012-self-package-export-target-rebinding.md)
already decided it, on this exact package and export, as a semantic dependency
on another artifact case rather than a local file edge, and that mechanism is
live. Both resolver variants were implemented — the located-root one in JS and
Rust together — and reverted; see the measurement for what they established.

The preceding baseline was [2026-09-08-prepared-selection-full-v2](2026-09-08-dependency-composition-lever.md),
418 probes, 324 complete, 62 partial, 23 refused, 9 not advanced, 1,151
certified entrypoints, 370 rows with the root certified. It added ADR 0070:
one row changed (`@kobalte/utils`, 19 → 22 entrypoints with its root) and 417
were identical.

The preceding baseline was [2026-09-08-positional-full-v2](2026-09-08-positional-origin-measurement.md),
418 probes, 324 complete, 62 partial, 23 refused, 9 not advanced, 1,148
certified entrypoints. It supersedes the `initial-reads-full` aggregate quoted
below and is the combined result of ADRs 0066, 0067, 0068 and 0069. Seven rows
gain, **411 of 418 are identical and nothing is lost**. Six of the gains are
ADRs 0066 and 0067, measured corpus-wide for the first time; the seventh is ADR
0068's [graph-fallback recovery](2026-09-08-graph-fallback-recovery.md).
ADR 0069 moves no row — see below.

Two operator facts belong with any rerun. `run.mjs`'s own
`DEFAULT_TIMEOUT_SECONDS` is 300 while every Makefile target passes
`--timeout 600`, and the recovery lane is an explicit reviewed
`--recover-probe` id list a new target must be added to.
`@kobalte/core@0.13.13` sits on the timeout boundary — 595.496 s of a 600 s
budget in the baseline, 600.014 s under contention, 378.394 s alone — so treat
its timeout as host load until a scoped rerun says otherwise.

ADR 0067's [ten-probe run](2026-09-08-first-iteration-recovery.md) then recovers
all three I18n roots and complete rows, preserving all 13 prior Marker/Motion/
Utils selections and claims. Full verification passes (240.40s). Six historical
losses are recovered in scoped runs; 24 remain outside that comparison.
A measured Kobalte Utils recovery attempt still refuses: the graph fallback
retries the whole unaccepted proposal instead of independently certifying its
cases. Correct that workflow while protecting previously published cases, then
run the full corpus. Exact compiler contexts and branch origins remain open.
**Both are now done** — see the current full baseline above. Kobalte Utils
publishes 19 of its 20 cases. Its twentieth, `scrollIntoViewport`, is the branch
origin, and [ADR 0069](../../adr/0069-original-input-by-position.md) supplies
that premise correctly and still does not close it: the demand takes the
whole-root arm, which quantifies over every read of the slot because an
`Operation` carries no source location, so no per-use row may answer it. **The
next bounded change on this path is an operation input's own site**, not another
origin premise — see the
[measurement](2026-09-08-positional-origin-measurement.md) for the diagnostic
that proves the arm and the two counterexamples that do not settle it.

Earlier baseline: `2026-09-07-implementation-owner-full.json`, 418 probes,
324 complete, 63 partial, 22 refused, 9 not advanced, 1,489 accepted artifact
selections. It preserves the previous full baseline's 1,489 selections. Locator
and Solid 2 Router's new complete-row status is entrypoint-name completeness;
their unproved conditional artifact cases remain explicit. This aggregate
precedes ADR 0064's subsequent parameter-read origin correction.

The subsequent [nine-probe correction measurement](2026-09-08-read-origin-coverage-regression.md)
loses seven artifact selections and regresses four complete rows. Both Solid 2
Motion rows retain `./m` but lose `.` and `./v2`; Utils `handleDiffArray` needs
positive evidence tied to each read's position relative to assignments. Restore
supportable coverage here before claiming additional complete-row gains. The
prior full aggregate is not a current post-correction measurement. ADR 0065's
[bounded opening-prefix proof](2026-09-08-initial-read-recovery.md) subsequently
restores all seven lost selections and four complete rows in the nine-probe
comparison. All 29 pre-regression selections and their exported claim sets
match, including in the completed full follow-up. That scoped preservation does
not extend to the remaining corpus, whose losses are recorded above.

## Search evidence and constraints

The repository search found that multi-case native finalization requires exact
equality between proposal artifact census and selected plans. Therefore partial
publication must project whole proposal cases and recertify, not weaken census
equality. ADR 0059 implements that approach.

The runner also omitted partial proposals without a complete dependency graph,
even when generation produced executable cases. Explicit recovery now submits
these to certification. This can yield new proofs, or new explicit refusals;
neither a scheduling advance nor a changed denominator alone is a certification.

External research consulted primary sources:

- [Node CommonJS namespaces](https://nodejs.org/api/esm.html#commonjs-namespaces):
  named CJS exports are detected by runtime-specific static analysis. Detection
  does not prove behavior, live updates or arbitrary dynamic exports.
- [TypeScript module reference](https://www.typescriptlang.org/docs/handbook/modules/reference):
  declaration substitution and module format govern which subject the compiler
  sees. A nearby differently formatted declaration is not interchangeable.
- [TypeScript ESM/CJS interoperability](https://www.typescriptlang.org/docs/handbook/modules/appendices/esm-cjs-interop.html):
  runtime and declaration interoperability can disagree. A new CJS lane must
  bind the execution model rather than copy ESM assumptions.

## Outstanding implementation tracks

Before widening parameter value-path certification, account for the
[accepted caller-read counterexample](2026-09-07-parameter-read-origin-investigation.md).
Exact binding references survive an unconditional replacement by a local object.
ADR 0064 now requires positive original-input evidence, and the counterexample
refuses. Full verification passes; coverage effects are measured separately.
Mixed caller/local origins still need their own premise. This prerequisite is
directly on the Floating UI coverage path; it is not general creates work.

1. **Independent case recovery and skipped partial proposals:** implemented
   and fully measured: 69 new artifact cases, eight new partial rows, no
   complete-row transition. The subsequent default-export census correction
   adds 68 cases in three probes. The combined full follow-up adds 651 cases
   across eight rows and verifies preservation of all 837 previous cases.
2. **Floating UI initialized member input:** the earlier investigation reached
   reassigned `list.concat` in three rows. The retry-sources full baseline still
   names it for Corvu Next Popover and Corvu; Corvu Popover first stops at a
   `sourceUnavailable` implementation transcript for nested
   `@corvu/utils@0.4.2`, `./create/controllableSignal`. Its runtime files are
   installed. Investigate exact program membership and installation ownership
   before changing flow proof. The implementation-location lookup currently
   selects the first materialized owner of matching snapshot bytes; that is a
   cause confirmed by ADR 0063's focused regression and scoped rerun.
   The own-installation fix clears `sourceUnavailable` and Corvu Popover now
   reaches Floating UI's same `list.concat` refusal. It remains refused, with
   no new accepted cases. The Floating UI path still needs source/assignment/
   control-flow evidence, not the unwritten-parameter premise.
3. **Callback flow:** Until and Intersection Observer have missing exact
   forwarding evidence. Inspect nested accepted-helper flow and observer
   constructor semantics separately; callback typing alone grants nothing.
   In the retry-sources report's published Rootless 1.5.4 catalog,
   `createBranch` has only `{"call":{},"shape":"callable"}`. The bundled
   contract carries a richer callback claim with lower bound zero. A new
   composition path must establish the exact applicable positive claim and its
   receipt authority; shape-only certification cannot supply that premise,
   and the bundled claim cannot be copied into another importer context.
4. **Recursive value shapes:** Flux Store, Local Store, Router and Solid 1
   direct-runtime entries expose generic/index/tuple census gaps. Distinguish
   source identities from actual callable or noncallable claims.
5. **CommonJS exports:** aria-query and partial-json block Testing Library and
   AI Solid; two Devtools packages also have no ESM surface. Needs authenticated
   module/export identity, source bindings and a supported execution model.
6. **Runtime libraries and module evaluation:** node:stream, worker event
   registration and effect-only modules need explicit behavior models and
   nonvacuous claims. Empty exports are not evidence of no behavior.
7. **Large/wildcard case sets:** binary subdivision is implemented for up to
   1,024 independent artifact cases, with a fresh final union transaction.
   Kobalte Core publishes 503 of 577 cases in the first scoped run, then 576
   after compiler-source retry isolation (ADR 0061), preserving all 503.
   ADR 0062 then recovers `./src/index.tsx` by binding its exact namespace
   entity rather than a same-named callable member: 577 accepted cases and
   508 entrypoint names. Full-corpus preservation passes, with no complete-row
   transition. Its 41 published test-module generation refusals remain explicit.
   Preserve wildcard,
   asset/type and intentionally scoped denominator distinctions; a resource
   deadline is not a semantic ceiling.
8. **External blockers:** missing published targets, invalid Solid 2 imports,
   missing declaration artifacts and SolidStart's application virtual module
   remain explicit. Do not fabricate a package version or application context.

This list is a work queue, not a claim that every listed track can certify its
target. Completion requires measured remaining blockers and a proof-based
ceiling audit, with no outstanding safe implementation path.
