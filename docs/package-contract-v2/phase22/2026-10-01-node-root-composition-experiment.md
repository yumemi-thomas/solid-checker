# A shared root proof advances a refusal but closes no additional export

This follow-up tests whether removing a shared Solid core call blocker yields
complete package coverage. The implementation is isolated on
`codex/composition-callables`; it is not promoted to the main engine or
accepted tier. Observations confer no certification authority.

## Result

Eleven affected published packages account for 75 Node export addresses.
The native catalogs report **12/75 clean before and after**. Every export
state is unchanged; no additional package becomes complete. Rootless also
remains 0/8 in both the host-free and browser controls, with identical export
states. This is thirteen native graph transactions over 91 host/export
addresses, not a new full 97-package checkpoint run.

| package | Node clean before | Node clean after | exports |
| --- | ---: | ---: | ---: |
| connectivity | 3 | 3 | 6 |
| keyboard | 2 | 2 | 7 |
| media | 0 | 0 | 6 |
| mouse | 2 | 2 | 8 |
| page-utilities | 2 | 2 | 4 |
| pointer | 0 | 0 | 7 |
| resize-observer | 0 | 0 | 7 |
| rootless | 0 | 0 | 8 |
| scroll | 2 | 2 | 6 |
| styles | 0 | 0 | 4 |
| url | 1 | 1 | 12 |

The shared rootless `createDisposable` and `createSubRoot` Reads refusals
advance past the previously unknown `solid-js.createRoot` call. They then
refuse `Array.forEach`, whose target is available only through a type rather
than exact runtime identity. The actual expression is
`asArray(access(owners)).forEach(...)`. Neither Reads domain closes. The same
two dependency exports recur in the eleven package transactions; counting
them eleven times would manufacture progress.

## What was tested

ADR 0178 adds a private audited-companion-archive guard to the existing
host-target reading. The new Node root Reads row requires:

- exact authenticated core rc.9 bytes and resolver replay selecting only
  `dist/server.js`;
- one distinct authenticated signals rc.9 archive, because the root's
  exception path tests an imported error class;
- existing callback attribution, applied to host-scoped Reads rows too.

No public claim form, value shape or producer protocol changes. Unknown,
ambiguous or changed companion archives refuse. Browser, host-free and
instrumented server bodies receive no new grant. The audit and exact source
slice are checked in on the experimental branch.

Two focused tests pass with certification pins armed. One checks resolution,
archive identity, missing/ambiguous dependencies and callback refusal. The
other runs the real producer, implementation census and mandatory runtime veto
on eight separate synthetic archives. Literal and original-frame parameter
callbacks close with nonempty veto roots. Captured helpers, owned accessors,
an owned option getter, namespace access and a parenthesized callee stay
uncertifiable. The option getter uses a real signals accessor even on Node.
The eighth control changes the imported error class's instance-check method
to read an owned signal. The planner refuses it, and the census also refuses
when supplied with the accessor bounds the real generator would propose.
Pinning a dependency does not silently promise an immutable runtime class.
All eight inputs pass strict TypeScript 5.9.3 against the complete published
rc.9 typings; there is no substituted Solid declaration stub or new diagnostic.

The namespace control reveals an unproved property-read form. The
parenthesized callee remains unresolved in the producer. Those refusals were
preserved and recorded, not silently treated as positive coverage.

## What this says about the strategy

Composition is technically workable: exact dependency claims and source
readings can remove individual blockers while adverse controls remain closed.
This measurement does not support scaling by writing more individual core
audits. A displayed first refusal hides further independent obligations, so
its frequency is not an estimate of clean-export gains. Here the apparent
shared wall disappears and the consumer-visible surface does not improve.

The more valuable next experiment must establish value provenance across
helpers, exact callable/member behavior, and end-to-end misuse detection.
The earlier batchEmits experiment already shows that a closed wrapper does
not by itself prove a read hidden in its returned caller object. The proposed
parameter-preservation claim remains unimplemented pending the owner's
decision under the rule requiring approval before new claim forms or shapes.
This experiment neither establishes that sound coverage of all packages is
impossible nor supplies evidence that the current approach will reach it.

## Evidence and verification

Retained observations under the main checkout's ignored build root:

- `rust/target/package-composition-node-root-node-2/results.json`;
- `rust/target/package-composition-node-root-{none,browser}-1/results.json`;
- `rust/target/package-composition-node-root-comparison.json`.

The comparison rereads the native publisher's named catalogs with digest
checks, rejects missing or extra exports, counts noncallable values, and
compares every export state to the preceding whole-corpus experiment. The
earlier `node-1` attempt is preserved: it omitted the registry-cache setting
and refused every offline acquisition. It is not semantic evidence.

Full `make verify` passes in the implementation worktree with the repository's
nextest runner: **TOTAL 647.55 seconds, exit 0, no `FAILED during step`**.
It runs 1,579 Rust tests plus one doctest, unchanged coverage (142 projects,
737 findings), ownership (41 cases / 465 ledger rows), 120 contract fixtures,
the TypeScript oracle, obligation audit, CLI, scripts, conformance, formatting
and workspace/all-target Clippy. Earlier attempts found three old Rust
audit-count assertions and one reporting-test count; all four were updated
for the new row and its byte citation.

The final class-mutation fixture was added after the full run. The strengthened
eight-input native test then passes in 31.78 seconds, and the additional input
passes strict published type checking. Full verification was not repeated for
that test-only addition; final formatting, workspace/all-target Clippy,
whitespace, schema JSON and dialect manifest checks cover the final tree.

No tracked accepted bundle, embedded tier ledger, finding snapshot, misuse
expectation or app metric changes. No tier regeneration or engine promotion.
The official checkpoint and criterion-3 counts remain unchanged. Nothing is
pushed.
