# async-carrier-provably-runs

**Claim (ADR 0563).** A pending async read in a callback a standard-library carrier runs is proven (SC5001, SC5003) only where the carrier provably runs the callback at least once (`carrier_runs::carrier_provably_runs`); an empty or unknown-length receiver makes the finding uncertifiable (`AsyncRead::carrier_run_unproven`), never silent. SC1001's strict read keeps its documented zero-run trade.

Violations: SC5001 in `BodyLiteralNonEmpty`, `BodySortTwo`, `BodyFromLiteral`; SC5003 in `JsxNonEmptyMap` (mounted, no Loading boundary).

Uncertifiable, never a violation (each a violation on 644bd1a81): SC5001 in `BodyEmptyNamed`, `BodyPropsReceiver`, `BodyEmptyLiteral`, `BodySetForEach`, `BodySortOne`; SC5003 in `JsxEmptyMap`.

Runtime: rc.17 throws `PENDING_ASYNC_UNTRACKED_READ` at each SC5001 violation and reads nothing on an empty receiver. Stub: `async-unsuspendable-call-attributed`'s. `tsc --noEmit` clean.
