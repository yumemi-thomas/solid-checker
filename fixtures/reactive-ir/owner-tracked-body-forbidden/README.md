# owner-tracked-body-forbidden

**Claim (ADR 0347, tracked bodies).** A `createTrackedEffect` body always runs under the tracked node, which forbids children, owned or not. So its `onCleanup` throws `CLEANUP_IN_FORBIDDEN_SCOPE` (SC3001), never `NO_OWNER_CLEANUP`, and a primitive there throws `PRIMITIVE_IN_FORBIDDEN_SCOPE` (SC3001). An unowned tracked effect keeps its own SC4001 (`NO_OWNER_EFFECT`).

Positives: `createTrackedEffect(() => onCleanup(() => {}))` at module scope, and `TimerTrackedEffect` (the same body in an unowned timer callback); each gives SC3001 at `onCleanup` and SC4001 at the callee.

Controls: a queued `onSettled` at module scope keeps SC4001 (`NO_OWNER_CLEANUP`); `TimerTrackedPrimitive` and `OwnedTrackedEffect` (SC3001 only).

Checked on the release runtime oracle, one site per case, with the codes above.

Stubs: copied from `owner-leaf-nested-cleanup`.
