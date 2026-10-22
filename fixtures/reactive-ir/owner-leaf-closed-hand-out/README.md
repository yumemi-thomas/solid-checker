# owner-leaf-closed-hand-out

**Claim (ADR 0347, named hand-outs in a closed program).** In a closed program an export a namespace import hands out by name has an unknown caller, so it is seeded with the open-world unowned bit, as an open program's export is. A leaf it creates is a caller obligation: uncertifiable, not a violation. Before the leaf edge kept the open-world bit, the `LEAF` bit made both `fnA` and `fnB` violations.

Uncertain: `fnA` (SC4001 and SC3001) and `fnB` (SC4001 and SC3001), handed out as `hand.fnA` and `hand.fnB`.

Silent: `fnC`, an export nothing hands out, which stays dormant.

Program boundary: `.solid-checker/runtime.json` declares `closed`.
