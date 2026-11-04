# flush-veto-kept

**Claim (ADR 0613).** Control of `flush-veto-*`: every `flush()` here runs with no owner (module scope, `setTimeout`, `runWithOwner(null, …)`, an `onClick` handler, after a dominating `await`) or drains nothing (in an effect apply, SC2008), and `Shadow.tsx`'s `flush` is a local function. So SC6002 and SC4001 stay violations and the writes in the apply and its returned cleanup stay silent. Probed (rc.17 fast runtime): a handler flush drains with no owner. Stubs as in `effect-phase-layers`.
