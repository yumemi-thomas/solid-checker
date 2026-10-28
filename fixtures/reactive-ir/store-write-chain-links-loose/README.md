# store-write-chain-links-loose

`store-write-chain-links` with `strict: false` (ADR 0465): Type Facts says the program does not apply strictNullChecks, so a link's type says nothing about its runtime value. `store.v = 1` stays a violation; every write through a link is uncertifiable. The nullable ones are where strict `tsc` would report TS18048/TS18047; here `tsc` is silent, and the result is internal only. No tsc-oracle case can pin this: its loose pass would need the checker silent (docs/open-work.md).
