# competing-check-sync-memo

**Claim (ADR 1052, part 2).** `createMemo(async, { sync: true })` reports SYNC_NODE_RECEIVED_ASYNC (SC7002) at its result probe, after every creation guard. Where an earlier check at the creation throws (an SC6002 violation or its `effect-owner-unproven` obligation: the effect-callback guard; an SC3001 violation written at the call: the leaf owner's refusal), SC7002 is uncertifiable `creation-throws-first`:

- `MemoInApply`, `NamespaceMemoInApply`: PRIMITIVE_IN_EFFECT_CALLBACK first.
- `MemoInTracked`, `MemoInSettled`: PRIMITIVE_IN_FORBIDDEN_SCOPE first (a call written in the leaf callback, every run of which is refused).

Part 5: `MemoInTrackedHelper`, `MemoInTrackedOnlyHelper` (a helper every reference to which is a refused call) and `MemoInGuardedRenderApply` (a render effect's apply the owned first run may leave before the creation) are uncertifiable. Violation kept: `MemoInBody`, `MemoInRenderApply` (straight line: the owned first run probes), `MemoInTrackedAndBodyHelper` (the body calls the helper first). The earlier checks keep their own rows.

Chrome: `fixtures/runtime-oracle/competing-check-sync-memo-*`. Stubs: `competing-check-first`'s layout, plus `createMemo` and `onSettled`.
