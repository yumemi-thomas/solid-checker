# Independent artifact case recovery

An unaccepted multi-case proposal can fail because one artifact case lacks a
proof even when other cases are certifiable. Explicit entrypoint recovery now
supports selecting those independent cases for a new value-only publication.

The complete proposal is tried first. Following a native proof refusal, at most
1,024 artifact cases may be considered, matching the contract document limit.
Every selection uses binary subdivision of refused batches, starting at the two
halves of the set that just refused. A successful batch supplies selection
hints only; the final union receives another complete native transaction,
including duplicate and conflict checks. Subdivision uses at most 2 × case
count native transactions including the initial and final transactions.

*Amended 2026-09-14.* Sets of 32 cases and under originally verified each
candidate together with every case selected so far. That growing prefix spent
one native transaction per case and re-certified every accepted case inside
each of them: the 31 retained cases of `solid-js@1.9.14` cost 33 transactions
and 292 Type Facts acquisitions, and that one row's serial chain was the
ecosystem corpus's 20-minute wall. All three selection paths — independent
recovery, the retained-proposal floor and its non-retained remainder, and the
ADR 0070 prepared-set selection — now share one subdivision
(`selectBySubdivision` in packages/cli/scripts/retained-proposal-graphs.mjs).
Trials of one selection may run side by side up to
`SOLID_CHECKER_RECOVERY_TRIAL_CONCURRENCY` (default 1, so trial order stays
deterministic; the ecosystem runner sets 4 for certification children); the
accepted and refused sets are sorted by case index, so the selection does not
depend on how trials were scheduled, and every trial still writes its own
private catalog and request file. Trial receipts remain hints only. Trial receipts never become inputs. Empty successful selections,
non-proof failures and final publication failures remain errors. A destination
that already existed at transaction entry cannot be replaced by a smaller
selection through this lane.

A subset proposal retains entire artifact cases and their referenced summary
definitions unchanged. Selection requires exact package name/version/integrity,
entrypoint, runtime and declaration paths/hashes, and both resolution branches.
Conflicting duplicate selections are rejected. Only unreferenced definitions
are omitted, as required by the existing document format. Native planning still
requires its complete selected census to equal the proposal artifact census;
there is no exception to that check. Snapshot replay, importer, conditions,
dependency receipts, trust, semantic proofs and ordinary consumer verification
remain the authority for every published case.

The diagnostic audit records expected and published coordinates plus explicit
per-case refusals. It is not receipt authority. Original generation refusals
remain separately visible. The existing coverage metric is unchanged; partial
condition coverage must not be called complete executable-artifact coverage.

The benchmark's explicit recovery option also queues partial proposals with a
positive generated-entrypoint count even when they have no dependency frontier.
The normal scheduling policy remains unchanged. An attempted row that refuses
is a measurement advance, not a new certification.

No shared protocol, receipt format or behavior model changes. This is fresh
certification of exact selections, not copying receipts between contexts.
