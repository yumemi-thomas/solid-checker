# ADR 0171: Flat reads rows require callback attribution

- Status: implemented and verified; refreshed tier authenticated
- Date: 2026-09-30
- Owner: `solid-facts-backend`, implementation census
- Relation: closes the by-reference gap recorded in ADR 0168

## Problem

A negative dialect row describes the audited archive's own code. It cannot
establish that an arbitrary function handed to that code belongs to the
caller. A package may create a lazy memo and pass its accessor to another
memo. The second call runs the first computation, which belongs to the
package under certification. Previously a flat `createMemo` reads row could
terminate the walk without accounting for that computation.

The retained rc.9 signals install reproduces this in all three published
builds: `prod/index.js`, `dev.js` and `observe/index.js`. Inside `createRoot`,
create a signal and a lazy memo that increments a counter and reads that
signal. The lazy memo leaves the counter at zero; `createMemo(local)` changes
it to one and reads the locally created signal. The same construction is
accepted by TypeScript 5.9.3 against `@solidjs/signals@2.0.0-rc.9`'s declarations,
with strict checking and no diagnostics. This is a certification gap, not a
new diagnostic rule.

## Decision

Before a flat negative reads row terminates a call, the census replays the
existing argument-attribution premises for every callback execution the
dialect models. It uses `callback_executions`, not `callback_positions`: the
latter is a rule-specific subset and omits an effect's compute slot.

A callback slot must be primitive by grammar, or attributable under ADR
0168: a callable whose locations lie inside the current transcript, or an
export parameter at depth zero. A module-level helper, an imported callable,
a created memo accessor, a displaced spread slot and absent facts refuse.
Every recognizing dialect's slots are checked; a disagreement cannot omit a
callback. Other domains retain their existing dispositions.

The same check applies to flat reads rows used as delegates. A delegate
without call-site premises refuses. The witness includes the checked callback
slots. Neither a new claim form nor a new value shape is introduced, and the
Type Facts protocol stays at 76.

## Validation and limits

The 147 focused Type Facts certification tests pass. Full `make verify`
passed in the isolated verification worktree, exit zero, TOTAL 936.14 seconds
and no `FAILED during step` marker. Controls cover a
literal's walked call, the caller's own parameter, an unattributed callable,
both effect callback slots, delegation and the absence of premises. Existing
ADR 0168 controls retain rejection of out-of-frame and cross-file locations
and parameters at a nested frame's depth.

This restriction does not widen the actual `solid-js` hydration/server
factory audits. It does not prove additional option callbacks, runtime hook
behavior, member dispatch or arbitrary callable identities. Structural
returns and application environment admission remain separate work.

## Tier refresh

The census, consumer-environment runs, retained three-host primitives
checkpoint and accepted-tier regeneration completed without clearing retained
inputs. Both census comparisons passed without repinning. All 30 census
probes certified in every host; consumer-environment refusals remain 1/23 for
Kobalte and 1/3 for Viviana, with both Oscar probes certified.

The rebuilt release retains 101 / 101 / 131 clean exports of 721. Tier
comparison finds the same 1,480 bundle identities and identical contract
documents: no added or removed bundle and no changed document. Refreshed
receipts and the 2,009 embedded objects authenticate. This fixes a latent
soundness gap and strengthens evidence; it is not a measured coverage gain.
The final release misuse rerun is identical to the previous ledger: two of
123 cases report correctly across their named hosts, and all 246 misuse and
correct-use snippets have zero TypeScript diagnostics against real published
typings. The checkpoint remains 1/97, with criteria 1/2/3 at 83/6/4 packages.
Post-tier `make test-rust`, formatting and workspace Clippy passed. Coverage
compared 141 fixture projects and 733 findings; the contract corpus checked
120 cases; ownership passed 41 cases with 465 ledger rows and none pending.
Contract conformance and `git diff --check` passed. Legacy conformance routes
contain zero active bundle cases; the separate focused authentication test is
the check over the actual accepted tier carried by this build.

The application metric was not rerun: the existing 1/1,850 result, including
zero of 292 primitives sites, remains the latest application measurement.
