# host-callback-listener-real

**Claim (ADR 0840, the negative of `host-callback-read-timing`'s move).** With no function of the program's own at `addEventListener`, a listener read on a host element keeps `host-callback-timing`: `SC1001` uncertifiable, `SC5001` for a pending read. `host-callback-read-timing` declares a project `addEventListener` method, so there the same calls are withheld (`standard-member-impostor`) and their reads are `callee-callback-timing`. Stub: `fresh-stack-scheduler-read-role`'s.

The components are exported: a function nothing renders is no proven component body (ADR 0445).
