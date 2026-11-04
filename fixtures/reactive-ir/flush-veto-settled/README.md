# flush-veto-settled

**Claim (ADR 0613).** `onSettled` called with no owner takes the effect queue (rc.17 `dist/dev.js:3058`), so a `flush()` under an owner drains its callback under that owner: rc.17 raises no `NO_OWNER_EFFECT` for the effect there and throws `REACTIVE_WRITE_IN_OWNED_SCOPE` for the write. Under the veto both are uncertifiable: neither a `missing-owner` violation nor a certified clean.
