# Coherent loading revisions and distinct evidence budgets

## Result

The prototype now preserves an issued revision when a file event leaves all
recorded analysis inputs unchanged. It also coalesces repeated pending reads
before applying the 64-record scope budget. Both changes improve conditional
informational feedback without adding a package-specific contract.

| Population | Previous target hints | Current target hints | Previous quiet controls | Current quiet controls |
| --- | ---: | ---: | ---: | ---: |
| Adapted loading-race replay, 7 stages | 0/3 | 3/3 | 4/4 | 4/4 |
| New registered queue budget cases, 34 | 6/14 | 12/14 | 20/20 | 18/20 |
| New directly returned reader cases, 34 | 14/14 | 14/14 | 16/20 | 16/20 |
| Previous async callback cases, 46 | 12/18 | 14/18 | 24/26 | 24/26 |
| Previous continuation cases, 36 | 10/15 | 11/15 | 19/19 | 19/19 |

There are four additional published-typing exclusions in the two older
populations. They receive no hints. Across these **157 variants**, independent
audits validate 446 observations, 174 async observations, three suppressions,
three nonempty retired batches and every plain behavior comparison. These are
authored cases and repeated stages over retained packages, not independent
real-app defects or a representative package sample.

The improvement exposes two constant-result warnings previously hidden by the
budget refusal. Quiet controls therefore decrease in that new population.
The earlier async callback set retains its two known constant-result noises.
Useful coverage improves; precision and universal package coverage are not
established.

## Unchanged events and real changes

The previous plugin invalidated any recorded path reported by the watcher,
even when the recorded source/configuration bytes were unchanged. A late
`tsconfig.json` event could give the helper a new issued generation while its
already served consumer retained the prior generation. Runtime attribution
correctly refused the mismatch, losing an observation.

Plugin V21 first validates the complete recorded input manifest. A valid
manifest preserves the session and cached consumer transforms. An invalid
manifest retires the revision and invalidates the transformed modules as
before. Plugin V22 combines that policy with the new runtime. Duplicate-event
statistics retain at most 64 file entries; excess entries increment a counter.

Browser V17 optionally invokes the actual update handler with an unchanged
configuration event immediately before the helper transform. Browser V18
replays the same hook with the previous V20 plugin/runtime V7. The application
code is identical. The hook is an integration reproducer, not a new production
watcher implementation.

Three bad stages retain their hints through initial loading, a helper edit
and a consumer comment edit. Three captured-value stages remain quiet and
update correctly. A retained constant-result control now collects its read
and suppresses it with the existing whole-result source proof; the baseline
was quiet because attribution was refused. Seven forced unchanged events
preserve current revisions; real edits produce different revisions and reject
old evidence. Eight focused tests additionally cover a genuine configuration
change, new included files, source changes and bounded event statistics.

This closes the demonstrated unchanged-input generation mismatch. Installed
dependency watching, concurrent module serving and stateful HMR remain open.
The browser harness uses actual full reloads with component HMR disabled.

## Repeated reads and distinct evidence

Runtime V8 indexes pending records by native identity/store key, read premise,
owner/observer context, original frames, helper/operation/completion lineage
and callback registration identity. Matching records accumulate an occurrence
count. Every original application read still executes. A representative
completed helper chain is retained; it is not a transcript of every invocation.

The 64-record bound applies to distinct pending records in a scope. Overflow
clears pending evidence and refuses the corresponding async continuation.
Throwing, unresolved completion, failed registration and explicit `untrack`
continue to refuse feedback. No owner/observer restoration, await or Promise
reaction is added. Global committed events, seen records and metadata are
still unbounded; this change does not establish long-session memory safety.

The registered population exercises serial and concurrent published queues:
30, 64, 70 and 200 reads of one source; 70 awaited child calls; and 64 or 65
different sources. Repeated targets retain one event with the exact count.
The 64-source cases retain 64 identities; 65-source cases refuse all pending
evidence. Captured-value fixes, terminal rejection and explicit-untrack
controls remain quiet. Constant results still lack a whole consumer-result
proof through the queue and emit informational hints.

The first new population, V1, accidentally exercised a different path: a
reader closure returned inside the memo was itself a candidate. Its synchronous
read could commit before the outer task returned or threw. The independent
audit found no async or callback-slot observations there. V2 uses a direct
reader alias to exercise registered continuation budgeting. V1 and its plain
and previous-version comparisons are preserved and reported separately.

That first population exposes four noises: serial/concurrent constant outputs
and caught terminal errors. It also admits 65 separately completed read scopes,
because the pending budget is per scope, not a global event cap. V2 was authored
after inspecting V1; it is a corrected mechanism challenge, not a blinded
holdout. Both populations use motivating package implementations already known
to the experiment.

## Boundaries and checks

Feedback remains `info`, with reactive intent open, static dispatch open and
`authority: false` / `certification: false`. Execution provenance does not
prove that a read influences the consumer's whole result. Unknown inputs and
unsupported dispatch/completion continue to leave analysis uncertifiable.
The existing projector V15 is reused; no production violation is added.

Historical modules stay immutable. New versions are runtime V8, callback-slot
runtime V3, continuation transform V5, plugins V21/V22 and browser V15–V18.
The new comparison validator supplements independent source/frame audit V11
with exact occurrence counts, per-scope refusal and unchanged/changed revision
assertions. It imports no detector. The detector was sealed before the new
populations; a separate handoff seal includes the populations and validator.

The prototype passes **674 tests with no skips**. The focused suites contain
eight loading tests and 60 callback/runtime tests. Actual `tsc --noEmit` with
published typings is clean for all 17 serial budget variants. Independent
audits reconstruct the public typing program for all browser variants. No
declaration stubs, package installs or network acquisition are used.

Syntax checks cover 466 modules. Authentication covers 47 historical/current
seals and 476 distinct pinned paths. Fast handoff checks pass: producer source
stamp, Rust formatting, pinned workspace Clippy, schema JSON, dialect manifests
and `git diff --check`.

Full production verification, fixture coverage, ownership and contract corpus
gates are deferred for this research-only slice. The older 23-case queue,
26-case cross-file and 40-case constant populations are not replayed here;
their previous results remain historical evidence. No production Rust source,
schema, contract, manifest, compiler lowering or finding snapshot changes.
Generated applications, traces and check outputs remain in ignored
`rust/target`.

## Remaining gaps

The previous async callback population still misses four object/Promise-adoption
targets. The continuation population still misses adopted child Promises,
nested reactions, object results and async generators. Distinct scope evidence
above 64 remains deliberately refused. Constant-result flow through packages,
the directly returned reader's result/error flow and intent remain open.

General callback storage, computed/mutable dispatch, synchronous-only installed
source, CommonJS, mixed transforms, prebundling, complete discovery, real-app
feedback precision and long-session cost remain open. The next meaningful
scalability measurement is actionable feedback and noise in ordinary applications;
larger authored benchmark totals alone will not establish that.

## Evidence

Repository-relative paths:

- `rust/target/semantic-update-browser-{reads,plain,baseline}-v1/results.json`.
- `rust/target/distinct-evidence-browser-{reads,plain,baseline}-v1/results.json`.
- `rust/target/registered-distinct-evidence-browser-{reads,plain,baseline}-v1/results.json`.
- `rust/target/distinct-evidence-prior-{async,continuation}-browser-reads-v1/results.json`.
- `rust/target/semantic-update-{audit,baseline-audit,comparison}-v1.json`.
- `rust/target/distinct-evidence-direct-{audit,baseline-audit,comparison}-v1.json`.
- `rust/target/registered-distinct-evidence-{audit,baseline-audit,comparison}-v1.json`.
- `rust/target/distinct-evidence-prior-{async,continuation}-audit-v1.json`.
- `rust/target/semantic-update-unit-v1.log`, `rust/target/distinct-evidence-unit-v1.log`
  and `rust/target/distinct-evidence-all-tests-v1.log`.
- `rust/target/distinct-evidence-tsc-v1.json`.
- `rust/target/distinct-evidence-{detector,handoff}-freeze-v1.json`,
  `rust/target/distinct-evidence-syntax-v1.json`,
  `rust/target/distinct-evidence-historical-seals-v1.json` and
  `rust/target/distinct-evidence-verify-fast-v1.log`.
- `rust/target/distinct-evidence-combined-summary-v1.json`.
