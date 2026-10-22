# owner-leaf-nested-gate

**Claim (ADR 0347, rc.13 queue).** An `onSettled` called from inside a leaf body has no children-capable owner (rc.13 queues it), so its callback runs with no owner: a memo or `flush()` there is not a forbidden primitive. The checker used to decide the gate `Owned` from the outer leaf's owned bit, a false SC3001 (`NestedMemo`, `NestedFlush`).

Negatives (no SC3001): `NestedMemo`, `NestedFlush`.

Control (SC3001): `OwnedMemo`, a memo in an owned `onSettled` body (`PRIMITIVE_IN_FORBIDDEN_SCOPE`).

Uncertain, not a violation: `HelperBothWays`, whose `onSettled` runs owned from the component body and out of band from a leaf.

Stubs: copied from `argument-sensitive-leaf`, with the `IntrinsicElements` shape widened.
