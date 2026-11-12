# owner-leaf-nested-gate

**Claim (ADR 0347, rc.13 queue).** An `onSettled` called from inside a leaf body has no children-capable owner (rc.13 queues it), so its callback runs with no owner: a memo or `flush()` there is not a forbidden primitive. The checker used to decide the gate `Owned` from the outer leaf's owned bit, a false SC3001 (`NestedMemo`, `NestedFlush`).

Negatives (no SC3001): `NestedMemo`, `NestedFlush`.

Control (SC3001): `OwnedMemo`, a memo in an owned `onSettled` body (`PRIMITIVE_IN_FORBIDDEN_SCOPE`).

Uncertain, not a violation: `HelperBothWays`, whose `onSettled` runs owned from the component body and out of band from a leaf.

Stubs: copied from `argument-sensitive-leaf`, with the `IntrinsicElements` shape widened.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
