# handed-net-guards

**Claim (ADRs 0770, 0771).** The programs the handed net must leave alone, or may only withhold. Quiet in Chrome, each one. `a10` (a parameter never called), `a12` (a closure nobody runs), `r8`, `c16` and `g2d-handler` (a handler), `r9` (a plain function in a memo), `g2d-empty` (a carrier that runs nothing) and `g2d-under-loading` stay certified: a net that reaches any of them has lost its reason. `n7` and `c15` are uncertifiable on purpose: the value is plain, and no fact at the call says so.
