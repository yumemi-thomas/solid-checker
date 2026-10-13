# Routing to a composing lane by the row's own refusal census

A package that re-exports a name from a dependency, or expands `export *`
through one, refuses at generation for want of an accepted contract the runner
never supplied. Two lanes answer that refusal by composing the dependency's own
receipt: the published-dependency-graph lane, which certifies exactly the
refused cases, and entrypoint recovery, which prepares the generated cases
*and* the frontier and publishes what proves.

Which rows reached those lanes was not a property of the rows. The graph lane
ran only under `--dependency-graph-lane`, and recovery only for probe ids named
one at a time on the command line. The reviewed list was 48 ids; the corpus had
51 rows whose refusal census named a dependency frontier, and 27 of them were
not on it. Five never asked for either lane and published their generated cases
with the frontier simply missing — `solid-js@2.0.0-rc.3`, three
`@tanstack/solid-start` probes and `@kobalte/core@2.0.0-alpha.0`, 34 refused
artifact cases between them.

A row that needs an accepted contract for a dependency is a row a composing
lane is the answer for, wherever it appears in the corpus. So the request is
now a policy over the row's own refusal census — `partial-success` plus a
dependency-composition refusal — and the reviewed id list keeps only the job it
alone can do: forcing recovery for a row with no dependency frontier, where the
lane answers a proof refusal instead. The queue that decides which rows are
certified at all follows the same census, so a row is never dropped on
scheduling grounds before any evidence is asked for.

The policy asks for recovery, never for the frontier-only graph lane. The two
are not ordered by coverage: the graph lane publishes the refused cases
*instead of* the generated ones, and a measured corpus lost receipts to exactly
that trade, which is why it was off by default. Recovery is the union, so it
dominates whenever there is anything to retain. A row whose generated
entrypoint count could not be read has nothing to retain, and keeps its reuse
rather than risking the trade on a number that is missing;
`--dependency-graph-lane` still selects the frontier-only lane by name.

Nothing about the evidence changes. The requested lane remains a preference —
`contract certify` decides, an unpreparable graph still falls back to the
proposal, and the lane a row reports is the one its audit recorded, never the
one the runner asked for.
