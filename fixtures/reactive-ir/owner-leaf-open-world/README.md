# owner-leaf-open-world

**Claim (ADR 0347, open-world seed).** An exported, non-component helper in an open program has a caller the checker cannot see, which may supply no owner. Its leaf's cleanup or effect is therefore a caller obligation: uncertifiable, not a violation. This is the same as the requirement written directly in the exported function. A leaf created by such a helper carries only the bare open-world bit, so the leaf edge keeps that bit (`LEAF` is not a proof).

Uncertain, not a violation: `helperCleanup` (SC4001 and SC3001), `helperEffect` (SC4001 and SC3001), and `helperShared`, which also has an owned caller `Comp` and is conditional.

Violation: `helperNested`. The inner `onSettled` is queued whatever owner the outer leaf has, so its `onCleanup` is proven unowned, as a timer callback in this helper would be; that claim does not depend on the caller. The outer leaf stays a caller obligation.

Violation with a proven path: `setup`, reached from an event handler (a proven unowned invocation) and from an owned call. The handler path makes the cleanup a violation; the owned path remains uncertain.

Stub declarations are copied from `argument-sensitive-leaf` (published rc.13 declarations), with the `IntrinsicElements` shape widened for the event handler.
