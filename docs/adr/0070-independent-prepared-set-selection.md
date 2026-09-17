# Independent selection across the prepared graph set

A package that re-exports a name from a dependency refuses at *generation*:
`@kobalte/utils`'s `./src/external.ts` says `export { Key } from
"@solid-primitives/keyed"`, and the runner generates the root proposal with no
dependency catalog, so `bindExport` has no accepted binding for `Key` and the
case never enters the proposal. Four of the row's cases refuse this way,
including the package root, which is why its root was uncertified.

Entrypoint recovery already answers this. `recoveryGraphCases` prepares the
retained proposal cases **plus** exactly those refused
dependency-composition cases, and the graph lane certifies them against an
authenticated dependency catalog. For `@kobalte/utils` that prepared set is 24
cases where the proposal had 20.

The set never got its chance. `certifyRecoverableCaseSelection` establishes the
retained cases as a baseline that every incremental trial extends, and a
baseline failure ended the strategy — correctly, because publishing less than
the proposal selected is not a silent liberty to take. But ending the strategy
also abandoned the graph, and the proposal it fell back to is the
dependency-blind one. One unprovable retained case
(`./src/scroll-into-view.ts`, an honest [ADR 0069](0069-original-input-by-position.md)
refusal) therefore took all four frontier cases down with it, and the row
published 19 of a possible 23.

Ending the incremental strategy is now distinct from abandoning the lane. When
the baseline refuses, the same bounded selection runs across the whole prepared
set — retained cases included, one native transaction each — and publishes what
proves. That is the [ADR 0068](0068-independent-generated-graph-fallback.md)
argument applied one level up, and it strictly dominates the proposal fallback:
the prepared set contains every case the proposal has and the frontier besides.

The one condition that makes publishing a subset safe is that nothing was
published here before, so the strategy is gated on the same
`existingPublication` capture both native lanes already share, threaded in as an
explicit tri-state: `false` asserts no prior publication, and `null` — a caller
that was not asked — keeps the original behaviour rather than reducing a
publication it cannot see. When the prepared set proves nothing at all, the
baseline refusal is rethrown so the proposal fallback still has its turn.

The audit says which strategy ran (`independent-prepared-selection`), what the
baseline refused with (`retainedBaselineRefusal`), the published coordinates,
and the exact per-case refusals. No claim inside a case is removed or weakened;
the selected union is freshly certified before publication, exactly as the other
selection paths do.

Focused tests cover the three routes: a refused retained case selecting
independently across the prepared set and publishing the frontier; an existing
publication — and an unstated one — never reduced; and a prepared set that
proves nothing rethrowing so the fallback runs. The existing retained-case,
duplicate, bound, non-proof-failure and final-publication protections are
unchanged.

This changes orchestration only. Contract, receipt, proof and trust interfaces,
the protocol and every snapshot are untouched.
