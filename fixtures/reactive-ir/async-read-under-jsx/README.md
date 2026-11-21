# async-read-under-jsx

**Claim (ADR 0614).** An effect's apply, or a cleanup it returns, is no render whichever JSX its effect is written in, so `async-outside-loading-boundary` does not claim a read there as rendered. Measured on rc.17: nothing reported.

ADR 1167: `CleanupInShow`'s read (a never-rerunning effect's returned cleanup, run at the branch's disposal) is clean for SC1001 too (it was uncertifiable).
