# prototype-write-escaped-helper

**Claim (ADR 0314), refusal.** `prototype-write-narrowed` with an alias of
the writing helper (`const store = OriginHelper.store`). Its calls are then
not all direct calls through the class's name, so the census cannot list the
prototypes it writes, and the write vetoes every exact-member proof as before
ADR 0314: every case keeps its `SC9012` uncertifiable finding.
