# Large graph recovery: first native result

Kobalte Core `2.0.0-alpha.0` publishes **122 artifact cases across 61
entrypoints**, up from 118 cases across 59 entrypoints in the retained full
baseline. The four new cases are `./colors` and `./i18n`, each under default
and `solid` export conditions. The [exact catalog inventory](2026-09-08-retained-graphs-kobalte-measurement.json)
follows the published case-set pointer and its named catalogs, checks their
digests, and records runtime/declaration hashes, export branches, importer
bindings, closure dependency identities and signed dependency receipt/trust
roots for each case. All four new cases have nonempty export claims.

All 118 old exact selections remain. Of those, 116 have identical claim sets;
both `createRegisterId` cases retain their previous claims and additionally
close `creates` with an empty creation set through the existing verifier.
This incidental strengthening is not counted as recovered entrypoint coverage.

The native run exited 0 after 113.861 seconds, using the current pinned verify
binary and producer, an exact retained installation, a fresh issuer and output
directory, graph preparation concurrency 1, and no network acquisition. There
were no archive/metadata cache misses. It prepared four semantic graph cases
and retained 118 ordinary proposal roots, then certified their 122-case union
in one native transaction. The audit records ordinary receipt authentication
and exact case selection. Log: `/private/tmp/retained-graph-kobalte2.log`;
retained result: `/private/tmp/retained-graph-native-P0diEC/result.json`.

The row remains **partial → partial** under the unchanged wildcard metric.
There is no new complete-row claim and no denominator correction. The
full-corpus result below confirms this gain without a row transition.

The earlier Solid 1 diagnostic exited 0 after 189.274 seconds through its
proposal fallback. Its graph path names `seroval-plugins@1.5.6 ./web`
`AbortSignalPlugin` as lacking a proven non-callable/non-constructable export
root, and its retained baseline names a missing `createResource` tuple-path
premise. It publishes 29 cases; no new Solid 1 coverage is claimed. Log:
`/private/tmp/retained-graph-solid1.log`; result:
`/private/tmp/retained-graph-native-MeS2VL/result.json`. This diagnostic used a
temporary candidate orchestrator while the baseline corpus was still running.

Focused validation: seven new tests, followed by the full CLI suite (207 tests
and TypeScript check), passed with exit 0. Full `make verify` passed with actual
exit 0, TOTAL 74.62 seconds, and no `FAILED during step` marker in
`/private/tmp/retained-graphs-verify.log`.

The post-change 418-probe run completed with exit 0 through
`/private/tmp/run-retained-graphs-full.mjs`, targeting
`2026-09-08-retained-graphs-full.json`. It retains artifacts, keeps the 900-second
probe timeout, uses generation concurrency 4 and certification concurrency 2,
and explicitly sets graph preparation concurrency 1. This concurrency differs
from the retained baseline; no timing improvement is claimed from that corpus
comparison. Log: `/private/tmp/retained-graphs-full.log`. It finished at
14:53:58 JST on September 8. The [full measurement](2026-09-08-retained-graphs-full-measurement.json)
records 324 complete, 62 partial, 23 refused and 9 not-advanced rows both before
and after. Artifact selections increase from 1,492 to 1,496; the only additions
are the four Kobalte cases above. There is no coverage-metric correction.

The [claim audit](2026-09-08-retained-graphs-all-claim-preservation.json)
follows named published catalogs for every previously certified row. All 1,492
old selections, including their closure identities, remain. Exactly 1,490
retain identical export claims; the two changes are solely the previously
described `createRegisterId` strengthening. Its strict claim-equality boolean
is therefore false, not a coverage-loss signal. Motion's accepted cases remain
present. No snapshots, bundled contracts, shared proof
interfaces or coverage metrics changed. No commit or push was made.
