# owner-leaf-nested-gate

**Claim (ADR 0347, rc.13 queue).** An `onSettled` called from inside a leaf body (another `onSettled`, a tracked effect, or a helper that runs in one) has no children-capable owner: rc.13 queues it. Its callback therefore runs with no owner, so a memo or `flush()` there is fine and is not a forbidden primitive. The checker used to decide the gate `Owned` from the outer leaf's owned bit and reported a false SC3001 (`NestedMemo` and `NestedFlush`).

Negatives (no SC3001): `NestedMemo` and `NestedFlush`.

Control (SC3001): `OwnedMemo`, a memo in an owned `onSettled` body, which throws PRIMITIVE_IN_FORBIDDEN_SCOPE.

Uncertain, not a violation: `HelperBothWays`, a helper whose `onSettled` runs owned from the component body and out of band from a leaf. Its gate is conditional, so the operation stays uncertifiable.

Stub declarations are copied from `argument-sensitive-leaf` (published rc.13 declarations), with the `IntrinsicElements` shape widened for the event handler.
