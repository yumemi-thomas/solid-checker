# optimistic-write-branch

**Claim (ADR 1216 part 2).** SC2009 says an optimistic setter **runs** before its async source's first answer. ADR 0636's
proof placed the call on the creating pass's stack but not that it runs: a write under a branch, after an early exit or in a
component's children was a violation Chrome does not confirm when that path does not run (main 60de70ac7: `AbsentProp`,
`PassedFalse`, `RuntimeFalse`, `DirectUnderBranch` static-only in the runtime oracle). The write and every call that carries
it must run whenever their function runs: a completion site (`completion_sites`), in no JSX element but an intrinsic one.

| component | SC2009 |
|---|---|
| `Straight`, `IntrinsicJsx` | violation |
| `AbsentProp`, `PassedFalse`, `RuntimeFalse`, `DirectUnderBranch`, `EarlyReturn`, `ComponentChildren` | uncertifiable (main: violation) |

Chrome: `fixtures/runtime-oracle/optimistic-write-branch`. Stub and `node_modules` from `optimistic-creating-entries`.
