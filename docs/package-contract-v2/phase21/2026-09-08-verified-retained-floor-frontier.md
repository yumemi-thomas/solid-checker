# A generated retained case is not necessarily an accepted floor

ADR 0077 advances the retained Solid 1 graph past the Seroval Plugins root
refusal. The next combined refusal is `createResource` at `./dist/dev.js`,
already refused by the ordinary proposal lane. The final accepted inventory is
still 29 artifact cases and 16 entrypoint names; all selections and claims are
preserved. See the
[exact measurement](2026-09-08-factory-return-solid1-measurement.json).

`certifyRetainedProposalSelection` currently treats all 31 generated retained
cases as mandatory. Two (`./dist/dev.js` and `./dist/solid.js`) are not among
the 29 independently certified cases. Their failure abandons all ten prepared
web graph cases. Those graph cases are not proved merely because this earlier
blocker moved; another refusal may remain once they are reached.

A bounded next strategy is to establish the retained floor through an ordinary
private proposal certification transaction after the all-retained graph trial
fails. Follow its published pointer and exact accepted cases, retain its
explicit case refusals, and match selections back to the original positional
inputs. Retry the graph union with every positively certified retained case
mandatory. If any of that verified floor fails in graph context, abandon the
strategy and retain the ordinary proposal result. Never use trial receipts as
inputs to graph publication; the final union must rebuild all proofs and
authenticate its own dependencies and trust.

The existing publication guard remains necessary: a destination that already
contains a publication must not have its floor reduced through a fresh trial.
Keep the full expected-case census, including both unproved raw cases, and
do not change the coverage denominator or reinterpret a missing record as a
refusal. Bound graph preparation at 32 missing cases and total publication at
1024 as before. Try the entire pruned union first to avoid one expensive graph
transaction per case when it succeeds.

Required controls: exact accepted/refused census partition; retained input and
coordinate identity; every positively accepted retained case preserved; an
unaccepted generated case cannot veto unrelated graph successes; incompatible
final unions and infrastructure failures refuse; pre-existing publications
retain their protection. Native publication and ordinary consumer verification
must precede any claimed gain. The bounded selection strategy is now implemented
in [ADR 0078](../../adr/0078-verified-retained-certification-floor.md), with
focused CLI controls.

The native measurement completed with actual exit 0 in 269.667 seconds and no
archive or metadata cache misses. Following the final pointer and named
catalogs gives 29 → 39 artifact cases and 16 → 20 entrypoint names. The ten
added cases are `./web` under import, development, browser,
browser+development, node, deno and worker conditions, plus
`./web/dist/dev.js`, `./web/dist/server.js` and `./web/dist/web.js` under import.
Every prior selection and semantic claim is preserved. The final transaction
authenticated receipts and ordinary exact case selection; private trial
receipts supplied no graph authority. Exact artifacts, closure hashes,
declarations, resolutions and receipt payloads are retained in the
[before/after measurement](2026-09-08-verified-retained-floor-solid1-measurement.json).

This is new certification, not a metric correction. The row remains partial:
the two raw Solid cases remain refused for missing positive tuple-path premises,
and `./web/types/index.d.ts` still cannot establish a runtime module under its
local `./client.js` specifier. Wildcard completeness is not inferred. The full
418-row corpus and Motion probes have not been rerun for this slice, so no new
aggregate or Motion preservation claim is made. Full `make verify` passed with
exit 0, TOTAL 76.30 seconds and no failed-step marker; all 214 CLI tests passed.
