# competing-check-sync-memo

**Claim (ADR 1052, part 2).** `createMemo(async, { sync: true })` reports SYNC_NODE_RECEIVED_ASYNC (SC7002) at its result probe, after every creation guard. Where an earlier check at the creation throws (an SC6002 violation or its `effect-owner-unproven` obligation: the effect-callback guard; an SC3001 violation written at the call: the leaf owner's refusal), SC7002 is uncertifiable `creation-throws-first`:

- `MemoInApply`, `NamespaceMemoInApply`: PRIMITIVE_IN_EFFECT_CALLBACK first.
- `MemoInTracked`, `MemoInSettled`: PRIMITIVE_IN_FORBIDDEN_SCOPE first (a call written in the leaf callback, every run of which is refused).

Violation kept: `MemoInBody`. Kept and open: `MemoInTrackedHelper` (a refusal through a helper refuses that caller's run only; a render that calls the helper first reports SC7002, the census's `settled-chain-placed`). The earlier checks keep their own rows.

Chrome: `fixtures/runtime-oracle/competing-check-sync-memo-*`. Stubs: `competing-check-first`'s layout, plus `createMemo` and `onSettled`.
