# owner-leaf-open-world

**Claim (ADR 0347, open-world seed).** An exported, non-component helper in an open program has a caller the checker cannot see, which may supply no owner. Its leaf's cleanup or effect is a caller obligation: uncertifiable, not a violation. A leaf such a helper creates carries only the bare open-world bit, which the leaf edge keeps (`LEAF` is not a proof).

Uncertain: `helperCleanup`, `helperEffect` (SC4001 and SC3001); `helperShared`, which also has an owned caller `Comp` and is conditional.

Violation: `helperNested`, whose inner `onSettled` is queued whatever owner the outer leaf has, so its `onCleanup` is proven unowned. Violation with a proven path: `setup`, reached from an event handler; its owned call path stays uncertain.

Stubs: copied from `argument-sensitive-leaf`, with the `IntrinsicElements` shape widened.
