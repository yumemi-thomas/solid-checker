# owner-leaf-nested-cleanup

**Claim (ADR 0347, rc.13 queue).** An `onSettled` created inside another leaf body (a tracked effect or an owned `onSettled`) takes the out-of-band queue: its callback runs with no owner, so an `onCleanup` there raises `NO_OWNER_CLEANUP` and never runs (SC4001). This holds for a helper that such a callback calls, too. The inner `onSettled` is never claimed as a forbidden call: its owner check queues it, it does not throw.

Positives (SC4001): `NestedInTracked` and `NestedInSettled` (the inner `onCleanup`), `registerSettled` reached from an owned `onSettled` (`HelperFromLeaf`), and `cleanupHelper` reached from a nested `onSettled` (`HelperFromNestedLeaf`).

Controls (no SC4001): `TrackedInTracked`, where a tracked effect inside a tracked effect throws PRIMITIVE_IN_FORBIDDEN_SCOPE before its body runs, so only that throw is claimed; `TrackedOwnCleanup`, where an owned tracked effect's own cleanup throws CLEANUP_IN_FORBIDDEN_SCOPE (SC3001); and `TrackedInSettled`, where a tracked effect inside an owned `onSettled` throws (SC3001) and its nested callback never runs.

Stub declarations are copied from `argument-sensitive-leaf` (published rc.13 declarations), with the `IntrinsicElements` shape widened for the event handler.
