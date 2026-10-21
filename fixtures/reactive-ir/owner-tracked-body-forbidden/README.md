# owner-tracked-body-forbidden

**Claim (ADR 0347, amendment of 2026-10-21, tracked bodies).** A `createTrackedEffect` body always runs under the tracked node, which forbids children, whether or not the effect was created under an owner. So an `onCleanup` there throws `CLEANUP_IN_FORBIDDEN_SCOPE` (SC3001), never `NO_OWNER_CLEANUP` (SC4001), and a primitive there throws `PRIMITIVE_IN_FORBIDDEN_SCOPE` (SC3001). The effect's own unowned creation keeps its SC4001 (`NO_OWNER_EFFECT`).

Positives: `createTrackedEffect(() => onCleanup(() => {}))` at module scope (SC3001 at `onCleanup`, SC4001 at the callee), and `TimerTrackedEffect`, the same body under an unowned timer callback (SC3001 at `onCleanup`, SC4001 at the callee).

Controls: a queued `onSettled` at module scope keeps SC4001 for its `onCleanup` (`NO_OWNER_CLEANUP`); `TimerTrackedPrimitive`, where `createEffect` in an unowned tracked body throws `PRIMITIVE_IN_FORBIDDEN_SCOPE` (SC3001, and no SC4001 for the primitive); and `OwnedTrackedEffect`, the owned case (SC3001 only).

Checked on the release runtime oracle, one site per case: the tracked-body cleanups raise `CLEANUP_IN_FORBIDDEN_SCOPE`, the timer primitive raises `PRIMITIVE_IN_FORBIDDEN_SCOPE`, the unowned tracked effects raise `NO_OWNER_EFFECT`, and the queued `onSettled` raises `NO_OWNER_CLEANUP`.

Stub declarations are copied from `owner-leaf-nested-cleanup` (published rc.13 declarations).
