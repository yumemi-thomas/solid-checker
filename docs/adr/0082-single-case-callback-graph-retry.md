# Single-case callback refusal through graph preparation

Until 0.1.1 generates a root proposal, then refuses its callback-flow demand.
There is no generation dependency-case refusal to trigger the graph lane.
An explicit graph request for the same retained root certifies its callable
export in 14.246 seconds, with no cache misses. Its callback domain stays open:
this is a new entrypoint certificate, not proof of the original callback claim.

Separate exact graph-case preparation from refusal-census selection. Do not
manufacture a generation refusal to request an existing case. Native archive,
lock, importer, dependency receipt, trust and publication checks are unchanged.

For opt-in dependency-graph or entrypoint-recovery requests, try the ordinary
proposal first. Permit one graph retry only when all these premises hold:

- the generated proposal selects exactly one artifact case;
- no publication directory existed before the ordinary attempt;
- the ordinary attempt refused at native witness acquisition as a certifier
  failure, naming an exact demand in `argument-binding` or `callable-path`.

A successful ordinary proposal never retries. Multiple cases, existing
publications, absent demand identities, other proof families and infrastructure
failures keep their existing result. A directory created by the failed attempt
is not a pre-existing publication. These boundaries prevent a graph result
from replacing accepted coverage: neither lane is a superset of the other.

The retry requests the exact original case coordinates. Its graph plans replace
the failed ordinary plans in the audit, and the audit retains the original
proposal digest, demand ID, family and refusal. Preparation failure preserves
the original refusal and records the preparation failure. Graph publication
remains a fresh native certification transaction; no trial receipt is reused.

Focused orchestration tests cover successful ordinary preservation, successful
retry, pre-existing publication, multiple cases, infrastructure failure, another
proof family and no opt-in. The ordinary CLI Until probe certifies in 14.518
seconds with zero cache misses and authenticated ordinary exact-case selection.
No native proof rule, public contract schema, receipt interface or metric is
changed. Complete-row gain must be measured from the published catalog; the
unproved callback operation is not represented as newly proved behavior.

The [scoped measurement](../package-contract-v2/phase21/2026-09-08-until-graph-recovery-measurement.json)
records an empty accepted set before and `{.}` after, with one artifact case:
`artifact-case:609e87d94104bc17ea20780fbca4b85a67432c1caf98b8c0b453c11c26324454`.
The unchanged legacy-main denominator establishes one refused-to-complete row.
The accepted summary is callable with an open call domain; callback execution
is still unproved and its original refusal remains in the audit.

Validation: 95 focused workflow tests pass, including the new seven-variant
orchestration regression. Full `make verify` exits 0 with TOTAL 79.60 seconds
and no failed-step marker (`/private/tmp/until-callback-retry-verify.log`).
An initial attempt to run the Vitest file through Node's test runner failed
before executing the suite; the reported focused result is from Vitest.
The full 418-row run, `2026-09-08-callback-retry-full.json`, finished at
21:06:43 JST on September 8 in 1,454.312 seconds with actual exit 0. Until was
added to the preceding recovery-probe set. The unchanged metric moves from
326 complete / 63 partial / 20 refused / 9 not advanced to **327 complete /
63 partial / 19 refused / 9 not advanced**. Until is the only row transition.

The [full measurement](../package-contract-v2/phase21/2026-09-08-callback-retry-full-measurement.json)
follows the published catalogs and records the exact before/after selections,
receipt envelopes and ordinary consumer verification. Until adds the same
root runtime/declaration/closure selection as the scoped measurement above,
with a fresh receipt bound to the full-run importer. Its original callback
refusal remains in `generatedProposalProofFallback`; its accepted call domain
remains open. No diagnostic receipt was copied into the full run.

The [all-claim audit](../package-contract-v2/phase21/2026-09-08-callback-retry-all-claim-preservation.json)
finds **1,532 → 1,533 artifact cases**, preserving every previous physical
selection and exported claim. The
[closure audit](../package-contract-v2/phase21/2026-09-08-callback-retry-closure-transitions.json)
finds zero closure identity transitions. Motion Solid 2 floor and head retain
`.`, `./m`, and `./v2`. This is one new certification, with no denominator or
coverage-metric correction. The runner's terminal generation totals are not
the certified-entrypoint totals above.

Matching native and producer binaries and the producer stamp are archived in
`rust/target/ecosystem-investigations/2026-09-08-callback-retry-binaries/`,
with a SHA-256 manifest checked against the measurement before any rebuild.
The three generic-result graph controls still refuse; Intersection Observer
also already refuses within the graph lane. Fractional Indexing's local
helper-read chain is an unimplemented positive-proof opportunity, not a
measured gain. Nothing was committed or pushed; no snapshots were changed.
