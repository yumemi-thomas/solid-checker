# owner-leaf-nested-cleanup

**Claim (ADR 0347, rc.13 queue).** An `onSettled` created inside another leaf body is queued out of band: its callback runs with no owner, so an `onCleanup` there raises `NO_OWNER_CLEANUP` and never runs (SC4001), including through a helper it calls. It is never a forbidden call: its owner check queues it, it does not throw.

Positives (SC4001): `NestedInTracked`, `NestedInSettled`, `HelperFromLeaf` (`registerSettled`), `HelperFromNestedLeaf` (`cleanupHelper`).

Controls (no SC4001): `TrackedInTracked` (throws `PRIMITIVE_IN_FORBIDDEN_SCOPE` before its body); `TrackedOwnCleanup` (SC3001); `TrackedInSettled` (throws, SC3001).

Stubs: copied from `argument-sensitive-leaf` (`IntrinsicElements` widened for the event handler).
