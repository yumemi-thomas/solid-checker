# Bounded runtime evidence and session scalability

## Outcome

The research runtime now bounds its owned observation history by record count
and accounted UTF-8 bytes. A browser trial completes 640 updates in each of
two retained queue modes with the same displayed result and 641 task calls as
the plain runs. One recent conditional read hint remains available in each
mode. Discarded observations are reported as partial coverage.

Compared with the preserved unbounded profile on identical consumers, retained
serialized evidence falls by 73–76%. This is a concrete improvement toward
long development sessions. It is not a total JavaScript heap bound, safe repair
claim, new application defect or general warning precision result.

## Retention policy

`native-read-runtime-v11.mjs` preserves the earlier observation semantics while
using `bounded-evidence-records-v1.mjs` for its strong history. Native node,
store and function identity remain weakly associated with application objects.
Recent duplicate observations retain their original identity and occurrence
count. Reobserving an evicted event starts a new retained occurrence count.

| Owned history/cache | Default record limit | Accounted byte limit |
| --- | ---: | ---: |
| Events and their deduplication keys | 256 | 4 MiB |
| Native metadata cache | 256 | 1 MiB |
| Callback metadata cache | 256 | 1 MiB |
| Refusal history | 128 | 1 MiB |
| Package guard identity cache | 128 | 1 MiB |

Accounting includes keys and encoded record data. Events reserve additional
space for occurrence-counter growth. Large metadata can be parsed without
caching; caching is an optimization. Oversized observations and guard identities
are refused with explicit counters and bounded gap records. The original
application value or exception is preserved.

Projector V21 validates the retention ledger and fails closed if it is missing,
malformed or inconsistent with the supplied snapshot. Valid retained positive
observations keep the earlier conditional feedback. Event/refusal history loss
and guard identity refusal produce an explicit open result. Metadata cache
eviction and guard cache eviction do not revoke already-issued evidence.
`observationCoverage.complete` remains false in every case.

Initial projectors V19/V20 and their frozen tests remain preserved. V20 separates
cache eviction from history loss; V21 additionally distinguishes a refused
guard identity, which drops a potential observation, from harmless guard-cache
eviction. The final browser profile is V25, using plugin V25, continuation
transform V8, callback runtime V6 and native runtime V11.

## Browser evidence

Both workloads use the retained published `@solid-primitives/queue` and
Solid/signals/web `2.0.0-rc.9`. No package bytes or application source are fixed.
The workloads are authored stress consumers with no failing value assertion.
Their conditional read hints measure feedback availability, not discovered
defects.

| Profile/population | Serial retained events | Concurrent retained events | Original observations per mode |
| --- | ---: | ---: | ---: |
| Preserved unbounded profile | 641 | 641 | 641 |
| Bounded V24 / first completed challenge | 171 | 152 | 641 |
| Final V25 / fresh renamed challenge | 170 | 152 | 641 |

For the first completed challenge, serialized keys plus event data decrease
from 15,701,090 to 4,181,463 bytes in serial mode and from 17,623,449 to
4,172,704 bytes in concurrent mode. That is a 73.4% and 76.3% reduction.
The final challenge accounts for 4,175,370 and 4,189,120 bytes, each below
the 4,194,304-byte event budget; it explicitly records 471 and 489 evictions.

Independent audits reconstruct event keys and encoded sizes, check distinct
registrations and exact source/native read provenance, and compare UI values,
task counts, diagnostics and published typing programs with plain runs. The
first and final stress populations each pass two plain comparisons. The
unbounded profile is also independently compared with the first plain runs.

An adapted eight-case replay preserves the earlier raw feedback: six target
hints and two noisy visible-counter controls. Retention does not solve result
relevance or developer intent. The separate failing-assertion feedback policy
and its prior calibration remain unchanged.

The first 512-cycle population was prepared before the final freeze and is
labeled adapted; it has no completed browser trial. The two completed
640-cycle populations were authored after their respective profile freezes.
Failed setup attempts stopped before browser execution and are not results.
Runs overlap development work; timings are not isolated performance evidence.

## Verification and artifacts

- All 823 prototype tests pass without skips: 734 preceding tests and 89 new
  bounded-runtime, adapted body-return and retention-projector checks.
- Four distinct new source variants pass actual `tsc --noEmit` invocations
  against published declarations, two from each completed stress population.
- Source/behavior audits validate both bounded populations, the unbounded
  comparison and the eight adapted feedback consumers.
- Independent retention audits validate all four bounded windows; the comparison
  audit authenticates the unchanged behavior and serialized-size reduction.

New modules are versioned successors under `benchmarks/reviewed-package-models/`.
Earlier modules remain immutable. Ignored `rust/target/` artifacts include the
three detector freezes, focused and full test logs, first/final typing reports,
`bounded-history-browser-{reads,plain}-v2/`,
`bounded-history-browser-unbounded-v2/`,
`bounded-history-final-{reads,plain}-v1/`,
`bounded-history-replay-reads-v1/`, and their browser, retention and comparison
audits. The handoff adds syntax, historical-seal and fast-verification records.

Fast handoff checks cover the producer stamp, Rust formatting, pinned workspace
Clippy, schema/manifest validation and whitespace. Full production verification,
coverage, ownership, contract and release gates remain deferred for this research
slice. No production Rust/CLI behavior, compiler lowering, snapshot, contract,
schema or manifest changes.

## Remaining work

Record count and encoded size bound the owned journals measured here. They do
not establish a bound for heap overhead, live weak identity associations, active
async scopes, application state, source programs or server instrumentation logs.
Startup cost and genuinely long real-application sessions still need measurement.

Older evidence is intentionally lost. Its absence must remain uncertifiable;
complete past execution coverage cannot be claimed from the retained window.
Broader package/source/dispatch coverage, result and error flow, genuine failing
application cases, safe repair and warning precision remain open. This change
addresses one practical scalability gap in the experimental runtime.
