# impostor-withheld-no-run

ADR 0874, R30-R33: a withheld call proves no run of its callback (`names`: the program's object typed as a list, whose
`forEach` queues it or hands it to `onCleanup`). Each site is uncertifiable in place: `HelperRead` SC1001 in the callback;
`ActionFlush` SC2006 at `flush()` (`provenStep`, on a real array, stays a violation); `RootWrite` SC2001. `Answered`: `reverse`
returns its receiver, so `xs.forEach` is uncertifiable (`impostor-host-intact` has the discarded answer).
