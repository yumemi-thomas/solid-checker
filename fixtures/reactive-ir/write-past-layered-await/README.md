# write-past-layered-await

**Claim (ADR 0611).** A write in a function that runs in place inside an async function (an IIFE, a helper calling its
callback, a named closure's one call) past that function's await runs on the later task with no owner, though its lexical
role is the enclosing compute's: legal (`Layered`; rc.17 reports nothing, fuzzer `memo-async`/`effect-async` legal cells,
16,528 programs that were false positives). Past an await on some paths only, it may run on the compute's first stack:
uncertifiable (`Branch`). A compute passed by name is the same (`Named`): the named tracked position is no proof past a
possible await. Controls stay violations: a `createRoot` body past the await runs under the root (rc.17 throws
REACTIVE_WRITE_IN_OWNED_SCOPE), and an IIFE before the await runs in the compute (`Controls`). The stub is copied from
`after-await-helper-callback` (rc.17).

**ADR 0899 (`HandedOn`).** A helper that calls its callback in place and also hands it on (a recursive `again(g, n - 1)`):
past the compute's await its in-place run is on the later task and its other runs are unfollowed, so the write is
uncertifiable, never the compute's violation (census `aw-forward-fan-run`: Chrome quiet, the checker gave SC2001 V). Before
the await the in-place run is in the compute: a violation, kept.
