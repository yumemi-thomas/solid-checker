# Large recovery sets: next experiment

The failed `2026-09-08-receipt-composition-full.json` run records
`@kobalte/core@2.0.0-alpha.0|solid2|only` being killed at the 4,096 MiB
process-tree ceiling after 444,817 ms of certification. Its generated proposal
has 59 entrypoint names but **118 artifact cases**, plus four refused frontier
cases (`./colors` and `./i18n`, each under default and `solid` conditions).
The current budget comment conflates entrypoint names with artifact cases.
That failed run has no retained artifacts or stage timings; it does not locate
the memory peak in preparation versus native acquisition.

The current implementation prepares semantic dependency graphs for every
generated case as well as the frontier. Native graph certification then
materializes one authenticated union and opens one Type Facts session. Its
complete plan list is also the runtime-binding owner lookup: removing plans
merely because their evidence was acquired earlier is not a safe optimization.

An alternative to test is to transport the original unaccepted generated cases
as independent roots, with exactly their ordinary authenticated compiler
sources, and prepare semantic graphs only for the missing frontier. The
existing schema-5 graph case-set request accepts roots with no semantic
dependencies; native planning still derives and refuses any unfulfilled
dependency demand. No receipt needs to be imported from an earlier transaction.
One fresh final transaction must authenticate the complete selected union and
ordinary consumer verification must select its exact cases.

This is an unmeasured hypothesis, not an implemented recovery or a reason to
raise the existing budget. The experiment must retain whole case claims,
artifact and declaration hashes, importer and condition identities, exact lock
selection, source packages and trust. It must reject duplicate/conflicting
selections and preserve the existing proposal fallback. Start with the larger
sets currently refused by the resource guard; the measured small-graph route
has additional certified cases and should remain a comparison control.

The live retained full run remains the fixed baseline. No CLI or native code
is changed while it runs. The next scoped experiment must use its named
retained artifacts and record memory failures, exact published case sets and
ordinary consumer verification, including all original accepted selections.
