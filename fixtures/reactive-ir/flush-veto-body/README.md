# flush-veto-body

**Claim (ADR 0613).** A `flush()` in a component body drains the queue under the component (rc.17 sets no owner around the drain), so every `createEffect` apply's ownerless proofs are withheld: SC6002 and SC4001 for the effect created in `CreatesInApply`'s apply, and SC2001 for the writes in `WritesInApply`'s apply and `WritesInReturnedCleanup`'s returned cleanup, are uncertifiable. Probed (rc.17 fast runtime): the inner effect raised nothing, the write threw `REACTIVE_WRITE_IN_OWNED_SCOPE`. Control: `flush-veto-kept`. Stubs as in `effect-phase-layers`.
