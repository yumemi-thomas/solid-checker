# Independent artifact case recovery

An unaccepted multi-case proposal can fail because one artifact case lacks a
proof even when other cases are certifiable. Explicit entrypoint recovery now
supports selecting those independent cases for a new value-only publication.

The complete proposal is tried first. Following a native proof refusal, at most
1,024 artifact cases may be considered, matching the contract document limit.
Up to 32 cases, each candidate is verified together with all cases selected so
far. Larger selections use binary subdivision of refused batches. A successful
batch supplies selection hints only; the final union receives another complete
native transaction, including duplicate and conflict checks. Subdivision uses
at most 2 × case count native transactions including the initial and final
transactions. Trial receipts never become inputs. Empty successful selections,
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
