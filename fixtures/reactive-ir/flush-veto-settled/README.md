# flush-veto-settled

**Claim (ADR 0613).** `onSettled` called with no owner takes the effect queue (rc.17 `dist/dev.js:3058`), so a `flush()` under an owner drains its callback under that owner: rc.17 raises no `NO_OWNER_EFFECT` for the effect there and throws `REACTIVE_WRITE_IN_OWNED_SCOPE` for the write. Under the veto both are uncertifiable: neither a `missing-owner` violation nor a certified clean.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
