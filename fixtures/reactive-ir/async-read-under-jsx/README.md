# async-read-under-jsx

**Claim (ADR 0614).** An effect's apply, or a cleanup it returns, is no render whichever JSX its effect is written in, so `async-outside-loading-boundary` does not claim a read there as rendered. Measured on rc.17: nothing reported.
